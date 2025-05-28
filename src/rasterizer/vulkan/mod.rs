use std::rc::Rc;

use ash::vk;
use winit::{event::WindowEvent, window::Window};

use crate::{scene::World, window::AppObserver};

use super::settings::Settings;

mod vulkan_base;
use vulkan_base::VulkanBase;
mod vulkan_swapchain;
use vulkan_swapchain::VulkanSwapchain;
mod vulkan_commands;
use vulkan_commands::VulkanCommands;
mod vulkan_descriptors;
mod vulkan_gui;
use vulkan_gui::VulkanGui;

#[cfg(feature = "stats")]
use super::Stats;

/// Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
pub struct VulkanEngine {
    // Elements are placed in the order they should be dropped, so inverse order of creation.
    gui: VulkanGui,
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
        self.swapchain.resize_if_necessary(&self.base);

        let current_frame = self.commands.current_frame();
        let device = &self.base.device;
        let image = self.swapchain.draw_img();

        current_frame.wait_for_fences();
        current_frame.begin_cmd_buf();
        current_frame.transition_image(
            device,
            *image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );

        current_frame.draw_background(&self.swapchain);

        self.gui.draw();

        let (swapchain_img_index, swapchain_image, sem_render) =
            self.swapchain.acquire_next_image(current_frame);
        current_frame.copy_img_swapchain(
            *image,
            self.swapchain.draw_extent(),
            *swapchain_image,
            self.swapchain.swapchain_extent,
        );
        current_frame.end_cmd_buf();
        current_frame.submit(sem_render, self.commands.queue);
        self.swapchain
            .present(swapchain_img_index, sem_render, self.commands.queue);

        self.commands.frame_number += 1;
    }

    pub fn new(window: Rc<Window>) -> Self {
        let base = VulkanBase::new(window);

        Self {
            gui: VulkanGui::new(&base),
            commands: VulkanCommands::new(&base),
            swapchain: VulkanSwapchain::new(&base),
            base,
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.gui.on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.gui.on_mouse_motion(delta);
    }
}
