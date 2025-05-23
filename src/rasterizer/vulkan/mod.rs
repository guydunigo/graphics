use std::{ops::DerefMut, rc::Rc};

use ash::vk;
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
    base: VulkanBase,
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
        let current_frame = self.commands.current_frame();
        // TODO: or deref device ?
        let device = &self.base.device;

        unsafe {
            device
                .wait_for_fences(&[current_frame.fence_render], true, 1_000_000_000)
                .unwrap();
            device.reset_fences(&[current_frame.fence_render]).unwrap();
        }

        self.swapchain.acquire_next_image(current_frame);
        // TODO: move
        let cmd_buf_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            device
                .reset_command_buffer(current_frame.cmd_buf, vk::CommandBufferResetFlags::empty())
                .unwrap();
            device
                .begin_command_buffer(current_frame.cmd_buf, &cmd_buf_begin_info)
                .unwrap();
        }

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
