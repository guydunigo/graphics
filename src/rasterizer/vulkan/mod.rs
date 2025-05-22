use std::{ops::DerefMut, rc::Rc, time::Duration};

use winit::{dpi::PhysicalSize, window::Window};

use crate::{font::TextWriter, scene::World, window::AppObserver};

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
    vulkan: VulkanBase,
    // TODO: window_extent: vk::Extent2D,
}

impl VulkanEngine {
    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        _settings: &Settings,
        _text_writer: &TextWriter,
        _world: &World,
        _buffer: &mut B,
        mut _size: PhysicalSize<u32>,
        _app: &mut AppObserver,
        #[cfg(feature = "stats")] _stats: &mut Stats,
    ) {
        std::thread::sleep(Duration::from_millis(16))
    }

    pub fn new(window: Rc<Window>) -> Self {
        let vulkan = VulkanBase::new(window);

        Self {
            commands: VulkanCommands::new(&vulkan),
            swapchain: VulkanSwapchain::new(&vulkan),
            vulkan,
        }
    }
}
