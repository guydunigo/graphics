use std::{ops::DerefMut, rc::Rc};

use winit::{dpi::PhysicalSize, window::Window};

use crate::{font::TextWriter, scene::World, window::AppObserver};

use super::settings::Settings;

mod vulkan_base;
use vulkan_base::VulkanBase;
mod vulkan_swapchain;
use vulkan_swapchain::VulkanSwapchain;

#[cfg(feature = "stats")]
use super::Stats;

/// Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
pub struct VulkanEngine {
    // TODO: convert to Rust way ?
    // frame_number: usize,
    // window_extent: vk::Extent2D,

    // Elements are placed in the order they should be dropped, so inverse order of creation.
    swapchain: VulkanSwapchain,
    vulkan: VulkanBase,
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
        // todo!();
    }

    pub fn new(window: Rc<Window>) -> Self {
        let vulkan = VulkanBase::new(window);
        let swapchain = VulkanSwapchain::new(&vulkan);
        Self {
            vulkan,
            swapchain,
            // Self::init_commands();
            // Self::init_sync_structures();
        }
    }

    fn init_commands(&mut self) {
        todo!();
    }

    fn init_sync_structures(&mut self) {
        todo!();
    }
}
