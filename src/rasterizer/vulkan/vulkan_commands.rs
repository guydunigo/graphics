use ash::{Device, vk};

use super::vulkan_base::VulkanBase;

const FRAME_OVERLAP: usize = 2;

#[derive(Debug)]
pub struct FrameData {
    cmd_pool: vk::CommandPool,
    pub cmd_buf: vk::CommandBuffer,

    pub fence_render: vk::Fence,
    pub sem_swapchain: vk::Semaphore,
}

impl FrameData {
    pub fn transition_image(
        &self,
        // TODO: store Device copy in FrameData ?
        device: &Device,
        image: &vk::Image,
        current_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        let aspect_mask = if new_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let sub_image = VulkanCommands::image_subresource_range(aspect_mask);

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
            .image(*image);

        let ibs = [image_barrier];
        let dep_info = vk::DependencyInfo::default().image_memory_barriers(&ibs);

        unsafe { device.cmd_pipeline_barrier2(self.cmd_buf, &dep_info) };
    }
}

pub struct VulkanCommands {
    device_copy: Device,

    pub queue: vk::Queue,
    frames: [FrameData; FRAME_OVERLAP],
    pub frame_number: usize,
}

impl VulkanCommands {
    pub fn new(base: &VulkanBase) -> Self {
        let queue = unsafe { base.device.get_device_queue(base.queue_family_index, 0) };

        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(base.queue_family_index);
        let fence_create_info =
            vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let sem_create_info = vk::SemaphoreCreateInfo::default();

        let frames: Vec<FrameData> = (0..FRAME_OVERLAP)
            .map(|_| {
                let cmd_pool = unsafe {
                    base.device
                        .create_command_pool(&pool_create_info, None)
                        .unwrap()
                };

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                    .command_buffer_count(1)
                    .command_pool(cmd_pool)
                    .level(vk::CommandBufferLevel::PRIMARY);

                // TODO: always take index 0 ?
                let cmd_buf = unsafe {
                    base.device
                        .allocate_command_buffers(&command_buffer_allocate_info)
                        .unwrap()
                }[0];

                let fence_render =
                    unsafe { base.device.create_fence(&fence_create_info, None).unwrap() };
                let sem_swapchain = unsafe {
                    base.device
                        .create_semaphore(&sem_create_info, None)
                        .unwrap()
                };

                FrameData {
                    cmd_pool,
                    cmd_buf,
                    fence_render,
                    sem_swapchain,
                }
            })
            .collect();
        let frames: [FrameData; FRAME_OVERLAP] = frames.try_into().unwrap();

        Self {
            // I hope it's okay to clone the device...
            // It's needed for Drop, but I'd like to keep this object separated from `VulkanBase`.
            device_copy: base.device.clone(),

            queue,
            frames,
            frame_number: 0,
        }
    }

    // TODO: to infinite iter ? .cycle() + self.frames.next().unwrap()
    pub fn current_frame(&self) -> &FrameData {
        &self.frames[self.frame_number % FRAME_OVERLAP]
    }

    pub fn image_subresource_range(aspect_mask: vk::ImageAspectFlags) -> vk::ImageSubresourceRange {
        vk::ImageSubresourceRange::default()
            .aspect_mask(aspect_mask)
            .base_mip_level(0)
            .level_count(vk::REMAINING_MIP_LEVELS)
            .base_array_layer(0)
            .layer_count(vk::REMAINING_ARRAY_LAYERS)
    }
}

impl Drop for VulkanCommands {
    fn drop(&mut self) {
        println!("drop VulkanCommands");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.frames.iter().for_each(|f| {
                self.device_copy.destroy_command_pool(f.cmd_pool, None);
                self.device_copy.destroy_fence(f.fence_render, None);
                self.device_copy.destroy_semaphore(f.sem_swapchain, None);
            });
        }
    }
}
