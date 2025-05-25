use std::rc::Rc;

use ash::vk;
use winit::window::Window;

use crate::{scene::World, window::AppObserver};

use super::settings::Settings;

mod vulkan_base;
use vulkan_base::VulkanBase;
mod vulkan_swapchain;
use vulkan_swapchain::VulkanSwapchain;
mod vulkan_commands;
use vulkan_commands::VulkanCommands;

#[cfg(feature = "stats")]
use super::Stats;

/// Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
pub struct VulkanEngine {
    // Elements are placed in the order they should be dropped, so inverse order of creation.
    commands: VulkanCommands,
    swapchain: VulkanSwapchain,
    base: VulkanBase,
    // TODO: window_extent: vk::Extent2D,
}

impl VulkanEngine {
    pub fn window(&self) -> &Rc<Window> {
        &self.base.window
    }

    pub fn rasterize(
        &mut self,
        _settings: &Settings,
        _world: &World,
        _app: &mut AppObserver,
        #[cfg(feature = "stats")] _stats: &mut Stats,
    ) {
        let current_frame = self.commands.current_frame();
        let device = &self.base.device;

        current_frame.wait_for_fences();

        let (swapchain_img_index, sem_render) = {
            let (swapchain_img_index, image, sem_render) =
                self.swapchain.acquire_next_image(current_frame);

            current_frame.begin_cmd_buf();
            current_frame.transition_image(
                device,
                image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
            );

            let flash = (self.commands.frame_number as f32 / 120.).sin().abs();
            let clear_value = vk::ClearColorValue {
                float32: [0., 0., flash, 1.],
            };
            let clear_range = VulkanCommands::image_subresource_range(vk::ImageAspectFlags::COLOR);
            unsafe {
                device.cmd_clear_color_image(
                    current_frame.cmd_buf,
                    *image,
                    vk::ImageLayout::GENERAL,
                    &clear_value,
                    &[clear_range],
                )
            };

            current_frame.transition_image(
                device,
                image,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );
            current_frame.end_cmd_buf();

            (swapchain_img_index, sem_render)
        };

        current_frame.submit(sem_render, self.commands.queue);
        self.swapchain
            .present(swapchain_img_index, sem_render, self.commands.queue);

        self.commands.frame_number += 1;

        // std::thread::sleep(Duration::from_millis(16))
    }

    pub fn new(window: Rc<Window>) -> Self {
        let base = VulkanBase::new(window);

        Self {
            commands: VulkanCommands::new(&base),
            swapchain: VulkanSwapchain::new(&base),
            base,
        }
    }
}
