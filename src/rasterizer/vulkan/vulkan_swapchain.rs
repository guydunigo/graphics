use std::{cell::RefCell, rc::Rc};

use ash::{Device, khr::swapchain, vk};
use vk_mem::Alloc;
use winit::dpi::PhysicalSize;

use super::{vulkan_base::VulkanBase, vulkan_commands::FrameData};

pub struct VulkanSwapchain {
    device_copy: Rc<Device>,

    window_size: PhysicalSize<u32>,
    is_suboptimal: RefCell<bool>,
    pub swapchain_loader: swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    images: Vec<(vk::Image, vk::ImageView, vk::Semaphore)>,

    draw_img: AllocatedImage,
    // TODO: draw_extent: vk::Extent2D,
}

impl VulkanSwapchain {
    pub fn new(base: &VulkanBase) -> Self {
        let present_mode = vk::PresentModeKHR::FIFO;
        // TODO: when implemented : MAILBOX : https://vkguide.dev/docs/new_chapter_1/vulkan_init_flow/
        // let present_mode = present_modes
        //     .iter()
        //     .cloned()
        //     .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
        //     .unwrap_or(vk::PresentModeKHR::FIFO);

        let surface_format = unsafe {
            base.surface_loader
                .get_physical_device_surface_formats(base.chosen_gpu, base.surface)
                .unwrap()[0]
        };

        let surface_capabilities = unsafe {
            base.surface_loader
                .get_physical_device_surface_capabilities(base.chosen_gpu, base.surface)
                .unwrap()
        };
        let mut desired_image_count = surface_capabilities.min_image_count + 1;
        if surface_capabilities.max_image_count > 0
            && desired_image_count > surface_capabilities.max_image_count
        {
            desired_image_count = surface_capabilities.max_image_count;
        }
        let window_size = base.window.inner_size();
        let surface_resolution = match surface_capabilities.current_extent.width {
            u32::MAX => vk::Extent2D {
                width: window_size.width,
                height: window_size.height,
            },
            _ => surface_capabilities.current_extent,
        };
        let pre_transform = if surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };

        let swapchain_loader = swapchain::Device::new(&base.instance, &base.device);
        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(base.surface)
            // TODO: should image format and color_space be checked ?
            .image_format(surface_format.format)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .present_mode(present_mode)
            .image_extent(surface_resolution)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .min_image_count(desired_image_count)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .image_array_layers(1);

        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap()
        };

        let sem_create_info = vk::SemaphoreCreateInfo::default();
        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain).unwrap() }
            .drain(..)
            .map(|image| {
                let create_view_info = image_view_create_info(
                    surface_format.format,
                    image,
                    vk::ImageAspectFlags::COLOR,
                );
                let view = unsafe {
                    base.device
                        .create_image_view(&create_view_info, None)
                        .unwrap()
                };

                let sem_render = unsafe {
                    base.device
                        .create_semaphore(&sem_create_info, None)
                        .unwrap()
                };

                (image, view, sem_render)
            })
            .collect();

        Self {
            // I hope it's okay to clone the device...
            // It's needed for Drop, but I'd like to keep this object separated from `VulkanBase`.
            device_copy: base.device.clone(),
            window_size,
            is_suboptimal: Default::default(),
            swapchain_loader,
            swapchain,
            images,
            draw_img: AllocatedImage::new(base, window_size),
        }
    }

    fn set_suboptimal(&self, is_suboptimal: bool) {
        if is_suboptimal {
            *self.is_suboptimal.borrow_mut() |= is_suboptimal;
        }
    }

    /// If window is resized, we need to recreate the whole swapchain.
    pub fn resize_if_necessary(&mut self, base: &VulkanBase) {
        // TODO: need to compare window_size or is_suboptimal is enough ?
        if *self.is_suboptimal.borrow() || self.window_size != base.window.inner_size() {
            *self = VulkanSwapchain::new(base);
        }
    }

    pub fn acquire_next_image(
        &self,
        current_frame: &FrameData,
    ) -> (u32, &vk::Image, &vk::Semaphore) {
        let (swapchain_img_index, is_suboptimal) = unsafe {
            self.swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    1_000_000_000,
                    current_frame.sem_swapchain,
                    vk::Fence::null(),
                )
                .unwrap()
        };
        self.set_suboptimal(is_suboptimal);

        let (i, _, s) = &self.images[swapchain_img_index as usize];
        (swapchain_img_index, i, s)
    }

    pub fn present(&self, swapchain_img_index: u32, sem_render: &vk::Semaphore, queue: vk::Queue) {
        let swapchains = [self.swapchain];
        let wait_semaphores = [*sem_render];
        let images_indices = [swapchain_img_index];
        let present_info = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .wait_semaphores(&wait_semaphores)
            .image_indices(&images_indices);
        let is_suboptimal = unsafe {
            self.swapchain_loader
                .queue_present(queue, &present_info)
                .unwrap()
        };
        self.set_suboptimal(is_suboptimal);
    }
}

impl Drop for VulkanSwapchain {
    fn drop(&mut self) {
        println!("drop VulkanSwapchain");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.images.drain(..).for_each(|(_, v, s)| {
                self.device_copy.destroy_image_view(v, None);
                self.device_copy.destroy_semaphore(s, None);
            });
        }
    }
}

fn image_create_info<'a>(
    format: vk::Format,
    usage_flags: vk::ImageUsageFlags,
    extent: vk::Extent3D,
) -> vk::ImageCreateInfo<'a> {
    vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(extent)
        .mip_levels(1)
        .array_layers(1)
        // for MSAA. we will not be using it by default, so default it to 1 sample per pixel.
        .samples(vk::SampleCountFlags::TYPE_1)
        // optimal tiling, which means the image is stored on the best gpu format
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(usage_flags)
}

fn image_view_create_info<'a>(
    format: vk::Format,
    image: vk::Image,
    aspect_flags: vk::ImageAspectFlags,
) -> vk::ImageViewCreateInfo<'a> {
    vk::ImageViewCreateInfo::default()
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        // .components(vk::ComponentMapping {
        //     r: vk::ComponentSwizzle::R,
        //     g: vk::ComponentSwizzle::G,
        //     b: vk::ComponentSwizzle::B,
        //     a: vk::ComponentSwizzle::A,
        // })
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .level_count(1)
                .layer_count(1)
                .base_mip_level(0)
                .base_array_layer(0)
                .aspect_mask(aspect_flags),
        )
        .image(image)
}

struct AllocatedImage {
    device_copy: Rc<Device>,
    allocator_copy: Rc<vk_mem::Allocator>,

    img: vk::Image,
    img_view: vk::ImageView,
    allocation: vk_mem::Allocation,
    extent: vk::Extent3D,
    format: vk::Format,
}

impl AllocatedImage {
    pub fn new(base: &VulkanBase, window_size: PhysicalSize<u32>) -> Self {
        let extent = vk::Extent3D {
            width: window_size.width,
            height: window_size.height,
            depth: 1,
        };
        // TODO: both to draw img
        let format = vk::Format::R16G16B16A16_SFLOAT;

        let draw_img_usages = vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::TRANSFER_DST
            | vk::ImageUsageFlags::STORAGE
            | vk::ImageUsageFlags::COLOR_ATTACHMENT;
        let rimg_info = image_create_info(format, draw_img_usages, extent);
        let mut rimg_allocinfo = vk_mem::AllocationCreateInfo::default();
        {
            // Prefered to GpuOnly (deprecated)
            rimg_allocinfo.usage = vk_mem::MemoryUsage::Auto;
            rimg_allocinfo.flags = vk_mem::AllocationCreateFlags::DEDICATED_MEMORY;
            rimg_allocinfo.priority = 1.;
            rimg_allocinfo.required_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        }

        let (img, allocation) = unsafe {
            base.allocator
                .create_image(&rimg_info, &rimg_allocinfo)
                .unwrap()
        };

        let view_create_info = image_view_create_info(format, img, vk::ImageAspectFlags::COLOR);
        let img_view = unsafe {
            base.device
                .create_image_view(&view_create_info, None)
                .unwrap()
        };

        Self {
            device_copy: base.device.clone(),
            allocator_copy: base.allocator.clone(),

            img,
            img_view,
            allocation,
            extent,
            format,
        }
    }
}

impl Drop for AllocatedImage {
    fn drop(&mut self) {
        println!("drop AllocatedImage");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.device_copy.destroy_image_view(self.img_view, None);
            self.allocator_copy
                .destroy_image(self.img, &mut self.allocation);
        }
    }
}

fn copy_img(
    device: &Device,
    cmd_buf: vk::CommandBuffer,
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

    unsafe { device.cmd_blit_image2(cmd_buf, &blit_info) };
}
