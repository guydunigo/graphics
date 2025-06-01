use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::{Device, vk};

use super::{
    vulkan_base::VulkanBase, vulkan_descriptors::ComputePushConstants,
    vulkan_swapchain::VulkanSwapchain,
};

pub const FRAME_OVERLAP: usize = 2;

pub struct FrameData {
    device_copy: Rc<Device>,

    pub cmd_pool: vk::CommandPool,
    pub cmd_buf: vk::CommandBuffer,

    fence_render: vk::Fence,
    pub sem_swapchain: vk::Semaphore,
}

impl Drop for FrameData {
    fn drop(&mut self) {
        println!("drop FrameData");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.device_copy.destroy_command_pool(self.cmd_pool, None);
            self.device_copy.destroy_fence(self.fence_render, None);
            self.device_copy.destroy_semaphore(self.sem_swapchain, None);
        }
    }
}

impl FrameData {
    pub fn new(
        device: Rc<Device>,
        pool_create_info: &vk::CommandPoolCreateInfo,
        fence_create_info: &vk::FenceCreateInfo,
        sem_create_info: &vk::SemaphoreCreateInfo,
    ) -> Self {
        let cmd_pool = unsafe { device.create_command_pool(pool_create_info, None).unwrap() };
        let cmd_buf = cmd_buffer(&device, cmd_pool);

        let fence_render = unsafe { device.create_fence(fence_create_info, None).unwrap() };
        let sem_swapchain = unsafe { device.create_semaphore(sem_create_info, None).unwrap() };

        Self {
            device_copy: device,
            cmd_pool,
            cmd_buf,
            fence_render,
            sem_swapchain,
        }
    }

    pub fn transition_image(
        &self,
        image: vk::Image,
        current_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        let aspect_mask = if new_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let sub_image = image_subresource_range(aspect_mask);

        // TODO: replace ALL_COMMANDS by more accurate masks to not stop whole GPU pipeline
        // https://github.com/KhronosGroup/Vulkan-Docs/wiki/Synchronization-Examples
        let image_barrier = vk::ImageMemoryBarrier2::default()
            .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
            .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
            .dst_access_mask(vk::AccessFlags2::MEMORY_WRITE | vk::AccessFlags2::MEMORY_READ)
            .old_layout(current_layout)
            .new_layout(new_layout)
            .subresource_range(sub_image)
            .image(image);

        let ibs = [image_barrier];
        let dep_info = vk::DependencyInfo::default().image_memory_barriers(&ibs);

        unsafe {
            self.device_copy
                .cmd_pipeline_barrier2(self.cmd_buf, &dep_info)
        };
    }

    pub fn wait_for_fences(&self) {
        unsafe {
            self.device_copy
                .wait_for_fences(&[self.fence_render], true, 1_000_000_000)
                .unwrap();
            self.device_copy.reset_fences(&[self.fence_render]).unwrap();
        }
    }

    pub fn begin_cmd_buf(&self) {
        begin_cmd_buf(&self.device_copy, self.cmd_buf);
    }

    pub fn end_cmd_buf(&self) {
        end_cmd_buf(&self.device_copy, self.cmd_buf);
    }

    pub fn submit(&self, sem_render: &vk::Semaphore, queue: vk::Queue) {
        let cmd_buf_submit_info =
            [vk::CommandBufferSubmitInfo::default().command_buffer(self.cmd_buf)];
        let wait_semaphore_info = [vk::SemaphoreSubmitInfo::default()
            .semaphore(self.sem_swapchain)
            .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT_KHR)
            .device_index(0)
            .value(1)];
        let signal_semaphore_info = [vk::SemaphoreSubmitInfo::default()
            .semaphore(*sem_render)
            .stage_mask(vk::PipelineStageFlags2::ALL_GRAPHICS)
            .device_index(0)
            .value(1)];

        let submit_info = vk::SubmitInfo2::default()
            .wait_semaphore_infos(&wait_semaphore_info)
            .signal_semaphore_infos(&signal_semaphore_info)
            .command_buffer_infos(&cmd_buf_submit_info);

        unsafe {
            self.device_copy
                .queue_submit2(queue, &[submit_info], self.fence_render)
                .unwrap()
        };
    }

    fn copy_img(
        &self,
        src: vk::Image,
        dst: vk::Image,
        src_size: vk::Extent2D,
        dst_size: vk::Extent2D,
    ) {
        let mut blit_region = vk::ImageBlit2::default();
        blit_region.src_offsets[1].x = src_size.width as i32;
        blit_region.src_offsets[1].y = src_size.height as i32;
        blit_region.src_offsets[1].z = 1;

        blit_region.dst_offsets[1].x = dst_size.width as i32;
        blit_region.dst_offsets[1].y = dst_size.height as i32;
        blit_region.dst_offsets[1].z = 1;

        blit_region.src_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
        blit_region.src_subresource.base_array_layer = 0;
        blit_region.src_subresource.layer_count = 1;
        blit_region.src_subresource.mip_level = 0;

        blit_region.dst_subresource.aspect_mask = vk::ImageAspectFlags::COLOR;
        blit_region.dst_subresource.base_array_layer = 0;
        blit_region.dst_subresource.layer_count = 1;
        blit_region.dst_subresource.mip_level = 0;

        let blit_regions = [blit_region];

        let blit_info = vk::BlitImageInfo2::default()
            .src_image(src)
            .src_image_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .dst_image(dst)
            .dst_image_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .filter(vk::Filter::LINEAR)
            .regions(&blit_regions);

        unsafe { self.device_copy.cmd_blit_image2(self.cmd_buf, &blit_info) };
    }

    /// draw_image must be in [`vk::ImageLayout::GENERAL`] and ends in [`vk::ImageLayout::TRANSFER_SRC_OPTIMAL`]
    /// swapchain_image ends in [`vk::ImageLayout::TRANSFER_DST_OPTIMAL`]
    pub fn copy_img_swapchain(
        &self,
        draw_image: vk::Image,
        draw_extent: vk::Extent2D,
        swapchain_image: vk::Image,
        swapchain_extent: vk::Extent2D,
    ) {
        self.transition_image(
            draw_image,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );
        self.transition_image(
            swapchain_image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        self.copy_img(draw_image, swapchain_image, draw_extent, swapchain_extent);
    }

    // pub fn draw_background_simple(&self, image: vk::Image, frame_number: usize) {
    //     let flash = (frame_number as f32 / 120.).sin().abs();
    //     let clear_value = vk::ClearColorValue {
    //         float32: [0., 0., flash, 1.],
    //     };
    //     let clear_range = VulkanCommands::image_subresource_range(vk::ImageAspectFlags::COLOR);
    //     unsafe {
    //         self.device_copy.cmd_clear_color_image(
    //             self.cmd_buf,
    //             image,
    //             vk::ImageLayout::GENERAL,
    //             &clear_value,
    //             &[clear_range],
    //         )
    //     };
    // }

    pub fn draw_background(
        &self,
        swapchain: &VulkanSwapchain,
        current_bg_effect: usize,
        current_bg_effect_data: &ComputePushConstants,
    ) {
        unsafe {
            self.device_copy.cmd_bind_pipeline(
                self.cmd_buf,
                vk::PipelineBindPoint::COMPUTE,
                swapchain.descriptors.bg_effects[current_bg_effect].pipeline,
            );
            self.device_copy.cmd_bind_descriptor_sets(
                self.cmd_buf,
                vk::PipelineBindPoint::COMPUTE,
                swapchain.descriptors.pipeline_layout,
                0,
                &[swapchain.descriptors.draw_img_descs],
                &[],
            );

            self.device_copy.cmd_push_constants(
                self.cmd_buf,
                swapchain.descriptors.pipeline_layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                current_bg_effect_data.as_u8_slice(),
            );

            let draw_extent = swapchain.draw_extent();
            self.device_copy.cmd_dispatch(
                self.cmd_buf,
                (draw_extent.width as f32 / 16.).ceil() as u32,
                (draw_extent.width as f32 / 16.).ceil() as u32,
                1,
            );
        }
    }
}

pub struct VulkanCommands {
    device_copy: Rc<Device>,

    pub allocator: Arc<Mutex<vk_mem::Allocator>>,

    pub queue: vk::Queue,
    frames: Vec<FrameData>,
    pub frame_number: usize,

    imm_fence: vk::Fence,
    imm_cmd_pool: vk::CommandPool,
    imm_cmd_buf: vk::CommandBuffer,
}

impl VulkanCommands {
    pub fn new(base: &VulkanBase, allocator: Arc<Mutex<vk_mem::Allocator>>) -> Self {
        let queue = unsafe { base.device.get_device_queue(base.queue_family_index, 0) };

        let pool_create_info = pool_create_info(base.queue_family_index);
        let fence_create_info = fence_create_info();
        let sem_create_info = vk::SemaphoreCreateInfo::default();

        let frames: Vec<FrameData> = (0..FRAME_OVERLAP)
            .map(|_| {
                FrameData::new(
                    base.device.clone(),
                    &pool_create_info,
                    &fence_create_info,
                    &sem_create_info,
                )
            })
            .collect();

        let imm_fence = unsafe { base.device.create_fence(&fence_create_info, None).unwrap() };
        let imm_cmd_pool = unsafe {
            base.device
                .create_command_pool(&pool_create_info, None)
                .unwrap()
        };
        let imm_cmd_buf = cmd_buffer(&base.device, imm_cmd_pool);

        Self {
            device_copy: base.device.clone(),

            allocator,

            queue,
            frames,
            frame_number: 0,

            imm_fence,
            imm_cmd_pool,
            imm_cmd_buf,
        }
    }

    pub fn current_frame(&self) -> &FrameData {
        &self.frames[self.frame_number % FRAME_OVERLAP]
    }

    pub fn immediate_submit<F: FnOnce(&Device, vk::CommandBuffer)>(&self, f: F) {
        let fences = [self.imm_fence];

        unsafe {
            self.device_copy.reset_fences(&fences[..]).unwrap();
        }
        begin_cmd_buf(&self.device_copy, self.imm_cmd_buf);

        f(&self.device_copy, self.imm_cmd_buf);

        end_cmd_buf(&self.device_copy, self.imm_cmd_buf);

        let cmd_buf_submit_info =
            [vk::CommandBufferSubmitInfo::default().command_buffer(self.imm_cmd_buf)];
        let submit_info = vk::SubmitInfo2::default().command_buffer_infos(&cmd_buf_submit_info);

        unsafe {
            // TODO: use different queue than graphics queue
            self.device_copy
                .queue_submit2(self.queue, &[submit_info], self.imm_fence)
                .unwrap();
            self.device_copy
                .wait_for_fences(&fences[..], true, 9_999_999_999)
                .unwrap();
        }
    }
}

impl Drop for VulkanCommands {
    fn drop(&mut self) {
        println!("drop VulkanCommands");
        unsafe {
            self.device_copy.destroy_fence(self.imm_fence, None);
            self.device_copy
                .destroy_command_pool(self.imm_cmd_pool, None);
        }
    }
}

fn fence_create_info<'a>() -> vk::FenceCreateInfo<'a> {
    vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED)
}

fn pool_create_info<'a>(queue_family_index: u32) -> vk::CommandPoolCreateInfo<'a> {
    vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
        .queue_family_index(queue_family_index)
}

fn cmd_buffer(device: &Device, cmd_pool: vk::CommandPool) -> vk::CommandBuffer {
    let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
        .command_buffer_count(1)
        .command_pool(cmd_pool)
        .level(vk::CommandBufferLevel::PRIMARY);
    // TODO: always take index 0 ?
    unsafe {
        device
            .allocate_command_buffers(&command_buffer_allocate_info)
            .unwrap()[0]
    }
}

fn begin_cmd_buf(device: &Device, cmd_buf: vk::CommandBuffer) {
    let cmd_buf_begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe {
        device
            .reset_command_buffer(cmd_buf, vk::CommandBufferResetFlags::empty())
            .unwrap();
        device
            .begin_command_buffer(cmd_buf, &cmd_buf_begin_info)
            .unwrap();
    }
}

fn end_cmd_buf(device: &Device, cmd_buf: vk::CommandBuffer) {
    unsafe { device.end_command_buffer(cmd_buf).unwrap() };
}

fn image_subresource_range(aspect_mask: vk::ImageAspectFlags) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(aspect_mask)
        .base_mip_level(0)
        .level_count(vk::REMAINING_MIP_LEVELS)
        .base_array_layer(0)
        .layer_count(vk::REMAINING_ARRAY_LAYERS)
}
