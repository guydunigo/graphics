use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::{Device, khr::swapchain, vk};
use vk_mem::Alloc;
use winit::dpi::PhysicalSize;

use super::{
    base::VulkanBase, commands::FrameData, compute_shaders::Effects, shaders_loader::ShadersLoader,
};

/// Creation of the swapchain and images based on the window.
pub struct VulkanSwapchain {
    inner: SwapchainData,

    draw_extent: vk::Extent2D,
    pub render_scale: f32,

    draw_img: AllocatedImage,
    depth_img: AllocatedImage,

    /// TODO: need to recreate on resize along swapchain because we use draw_img ?
    pub effects: Effects,
}

impl VulkanSwapchain {
    /// If `min_extent` is provided, the new image will be at least as big in height and/or in
    /// width.
    pub fn new(
        base: &VulkanBase,
        shaders: &ShadersLoader,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        min_extent: Option<vk::Extent3D>,
    ) -> Self {
        let swapchain_data = SwapchainData::new(base);
        let window_size = base.window.inner_size();
        let max_size = min_extent
            .map(|e| PhysicalSize {
                width: u32::max(window_size.width, e.width),
                height: u32::max(window_size.height, e.height),
            })
            .unwrap_or(window_size);

        let draw_img = AllocatedImage::new_draw(base.device.clone(), allocator.clone(), max_size);
        let depth_img = AllocatedImage::new_depth(base.device.clone(), allocator, max_size);

        let effects = Effects::new(base.device.clone(), shaders, draw_img.img_view);

        let draw_extent = vk::Extent2D {
            width: draw_img.extent.width,
            height: draw_img.extent.height,
        };

        Self {
            inner: swapchain_data,
            draw_extent,
            render_scale: 1.,
            draw_img,
            depth_img,
            effects,
        }
    }

    fn set_suboptimal(&self) {
        println!("Suboptimal swapchain, needs resizing.");
        *self.inner.is_suboptimal.borrow_mut() = true;
    }

    fn set_out_of_date_khr(&self) {
        println!("Error out of date khr, needs, resizing.");
        *self.inner.is_suboptimal.borrow_mut() = true;
    }

    /// If window is resized, we need to recreate the swapchain.
    ///
    /// If the new size is smaller, we only recreate the [`SwapchainData`],
    /// but if it's bigger we recreate the [`VulkanSwapchain`] and re-allocate the images at least
    /// as big.
    pub fn resize_if_necessary(
        &mut self,
        base: &VulkanBase,
        shaders: &ShadersLoader,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
    ) {
        let window_size = base.window.inner_size();

        if *self.inner.is_suboptimal.borrow()
            || *self.inner.is_out_of_date_khr.borrow()
            || self.inner.window_size != window_size
        {
            // If the draw_img is bigger, we avoid re-allocating it,
            // and just use smaller extent (updated from [`update_draw_extent`]) :
            if self.draw_img.extent.height >= window_size.height
                && self.draw_img.extent.width >= window_size.width
            {
                println!("--- Resize swapchain only ---");
                self.inner = SwapchainData::new(base);
            } else {
                println!("--- Resize swapchain and draw image ---");
                *self = VulkanSwapchain::new(base, shaders, allocator, Some(self.draw_img.extent));
            }

            println!("--- End of resize ---");
        }

        self.update_draw_extent();
    }

    pub fn acquire_next_image(
        &self,
        current_frame: &FrameData,
    ) -> Option<(u32, &vk::Image, &vk::Semaphore, &vk::ImageView)> {
        let res = unsafe {
            self.inner.swapchain_loader.acquire_next_image(
                self.inner.swapchain,
                1_000_000_000,
                current_frame.sem_swapchain,
                vk::Fence::null(),
            )
        };

        match res {
            Ok((swapchain_img_index, is_suboptimal)) => {
                if is_suboptimal {
                    self.set_suboptimal();
                }

                let (i, v, s) = &self.inner.swapchain_images[swapchain_img_index as usize];

                Some((swapchain_img_index, i, s, v))
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.set_out_of_date_khr();
                None
            }
            Err(e) => panic!("Error acquiring next image : {e}"),
        }
    }

    pub fn present(&self, swapchain_img_index: u32, sem_render: &vk::Semaphore, queue: vk::Queue) {
        let swapchains = [self.inner.swapchain];
        let wait_semaphores = [*sem_render];
        let images_indices = [swapchain_img_index];
        let present_info = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .wait_semaphores(&wait_semaphores)
            .image_indices(&images_indices);
        let res = unsafe {
            self.inner
                .swapchain_loader
                .queue_present(queue, &present_info)
        };

        match res {
            Ok(false) => (),
            Ok(true) => self.set_suboptimal(),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.set_out_of_date_khr(),
            Err(e) => panic!("Error presenting queue : {e}"),
        }
    }

    fn update_draw_extent(&mut self) {
        self.draw_extent.height = (u32::min(
            self.inner.swapchain_extent.height,
            self.draw_img.extent.height,
        ) as f32
            * self.render_scale) as u32;
        self.draw_extent.width = (u32::min(
            self.inner.swapchain_extent.width,
            self.draw_img.extent.width,
        ) as f32
            * self.render_scale) as u32;
    }

    pub fn draw_extent(&self) -> vk::Extent2D {
        self.draw_extent
    }

    pub fn swapchain_extent(&self) -> vk::Extent2D {
        self.inner.swapchain_extent
    }

    pub fn swapchain_img_format(&self) -> vk::Format {
        self.inner.swapchain_img_format
    }

    pub fn draw_img(&self) -> &vk::Image {
        &self.draw_img.img
    }

    pub fn draw_img_view(&self) -> &vk::ImageView {
        &self.draw_img.img_view
    }

    pub fn draw_format(&self) -> &vk::Format {
        &self.draw_img.format
    }

    pub fn depth_img(&self) -> &vk::Image {
        &self.depth_img.img
    }

    pub fn depth_img_view(&self) -> &vk::ImageView {
        &self.depth_img.img_view
    }

    pub fn depth_format(&self) -> &vk::Format {
        &self.depth_img.format
    }
}

/// Part that needs to be recreated on resize
struct SwapchainData {
    device_copy: Rc<Device>,

    window_size: PhysicalSize<u32>,
    is_suboptimal: RefCell<bool>,
    is_out_of_date_khr: RefCell<bool>,

    pub swapchain_loader: swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<(vk::Image, vk::ImageView, vk::Semaphore)>,
    pub swapchain_img_format: vk::Format,
    pub swapchain_extent: vk::Extent2D,
}

impl SwapchainData {
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
        let swapchain_img_format = surface_format.format;

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
        let swapchain_extent = match surface_capabilities.current_extent.width {
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
            .image_format(swapchain_img_format)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .present_mode(present_mode)
            .image_extent(swapchain_extent)
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
        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain).unwrap() }
            .drain(..)
            .map(|image| {
                let create_view_info = image_view_create_info(
                    swapchain_img_format,
                    image,
                    vk::ImageAspectFlags::COLOR,
                    None,
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
            device_copy: base.device.clone(),
            window_size,
            is_suboptimal: Default::default(),
            is_out_of_date_khr: Default::default(),
            swapchain_loader,
            swapchain,
            swapchain_images,
            swapchain_img_format,
            swapchain_extent,
        }
    }
}

impl Drop for SwapchainData {
    fn drop(&mut self) {
        println!("drop SwapchainData");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.swapchain_images.drain(..).for_each(|(_, v, s)| {
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
    mip_level: Option<u32>,
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
                .base_mip_level(mip_level.unwrap_or(0))
                .base_array_layer(0)
                .aspect_mask(aspect_flags),
        )
        .image(image)
}

pub struct AllocatedImage {
    device_copy: Rc<Device>,
    allocator_copy: Arc<Mutex<vk_mem::Allocator>>,

    img: vk::Image,
    img_view: vk::ImageView,
    allocation: vk_mem::Allocation,
    extent: vk::Extent3D,
    format: vk::Format,
}

impl AllocatedImage {
    pub fn new_draw(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        window_size: PhysicalSize<u32>,
    ) -> Self {
        let usages = vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::TRANSFER_DST
            | vk::ImageUsageFlags::STORAGE
            | vk::ImageUsageFlags::COLOR_ATTACHMENT;

        Self::new_with_window_size(
            device,
            allocator,
            window_size,
            vk::Format::R16G16B16A16_SFLOAT,
            usages,
            vk::ImageAspectFlags::COLOR,
        )
    }

    pub fn new_depth(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        window_size: PhysicalSize<u32>,
    ) -> Self {
        Self::new_with_window_size(
            device,
            allocator,
            window_size,
            vk::Format::D32_SFLOAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::ImageAspectFlags::DEPTH,
        )
    }

    pub fn new_with_window_size(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        window_size: PhysicalSize<u32>,
        format: vk::Format,
        usages: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
    ) -> Self {
        let extent = vk::Extent3D {
            width: window_size.width,
            height: window_size.height,
            depth: 1,
        };
        Self::new(device, allocator, extent, format, usages, aspect)
    }

    fn new(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        extent: vk::Extent3D,
        format: vk::Format,
        usages: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
    ) -> Self {
        let rimg_info = image_create_info(format, usages, extent);
        let mut rimg_allocinfo = vk_mem::AllocationCreateInfo::default();
        {
            // Example : https://gpuopen-librariesandsdks.github.io/VulkanMemoryAllocator/html/usage_patterns.html
            // Prefered to GpuOnly (deprecated)
            rimg_allocinfo.usage = vk_mem::MemoryUsage::Auto;
            rimg_allocinfo.flags = vk_mem::AllocationCreateFlags::DEDICATED_MEMORY;
            rimg_allocinfo.priority = 1.;
            rimg_allocinfo.required_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        }

        let (img, allocation) = unsafe {
            allocator
                .lock()
                .unwrap()
                .create_image(&rimg_info, &rimg_allocinfo)
                .unwrap()
        };

        let view_create_info = image_view_create_info(format, img, aspect, None);
        let img_view = unsafe { device.create_image_view(&view_create_info, None).unwrap() };

        Self {
            device_copy: device,
            allocator_copy: allocator,

            img,
            img_view,
            allocation,
            extent,
            format,
        }
    }

    // TODO: names
    pub fn new_image(
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        extent: vk::Extent3D,
        format: vk::Format,
        usages: vk::ImageUsageFlags,
        mipmapped: bool,
    ) -> Self {
        let mut img_info = image_create_info(format, usages, extent);
        if mipmapped {
            img_info.mip_levels =
                f32::floor(f32::log2(u32::max(extent.width, extent.height) as f32)) as u32 + 1;
        }

        let mut alloc_info = vk_mem::AllocationCreateInfo::default();
        alloc_info.usage = vk_mem::MemoryUsage::Auto;
        alloc_info.required_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;

        unsafe {
            allocator
                .lock()
                .unwrap()
                .create_image(&img_info, &alloc_info)
                .unwrap();
        }

        let aspect = if format == vk::Format::D32_SFLOAT {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let new_image = Self::new(device.clone(), allocator, extent, format, usages, aspect);

        let view_info =
            image_view_create_info(format, new_image.img, aspect, Some(img_info.mip_levels));

        unsafe {
            device.create_image_view(&view_info, None).unwrap();
        }

        new_image
    }
}

impl Drop for AllocatedImage {
    fn drop(&mut self) {
        println!("drop AllocatedImage");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.device_copy.destroy_image_view(self.img_view, None);
            self.allocator_copy
                .lock()
                .unwrap()
                .destroy_image(self.img, &mut self.allocation);
        }
    }
}
