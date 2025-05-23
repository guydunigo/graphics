use ash::{Device, khr::swapchain, vk};

use super::{vulkan_base::VulkanBase, vulkan_commands::FrameData};

const IMAGE_FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;

pub struct VulkanSwapchain {
    device_copy: Device,

    pub swapchain_loader: swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    // TODO: separé de views ou vec de tuples ?
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
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
            .image_format(IMAGE_FORMAT)
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

        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain).unwrap() };
        let image_views: Vec<vk::ImageView> = images
            .iter()
            .map(|&image| {
                let create_view_info = vk::ImageViewCreateInfo::default()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(IMAGE_FORMAT)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .level_count(1)
                            .layer_count(1),
                    )
                    .image(image);
                unsafe {
                    base.device
                        .create_image_view(&create_view_info, None)
                        .unwrap()
                }
            })
            .collect();

        Self {
            // I hope it's okay to clone the device...
            // It's needed for Drop, but I'd like to keep this object separated from `VulkanBase`.
            device_copy: base.device.clone(),
            swapchain_loader,
            swapchain,
            images,
            image_views,
        }
    }

    pub fn acquire_next_image(&self, current_frame: &FrameData) -> (u32, &vk::Image) {
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
        assert!(
            !is_suboptimal,
            "Swapchain is suboptimal and no longer matches the surface properties exactly, see VK_SUBOPTIMAL_KHR"
        );
        (
            swapchain_img_index,
            &self.images[swapchain_img_index as usize],
        )
    }
}

impl Drop for VulkanSwapchain {
    fn drop(&mut self) {
        println!("drop VulkanSwapchain");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.image_views
                .drain(..)
                .for_each(|v| self.device_copy.destroy_image_view(v, None));
        }
    }
}
