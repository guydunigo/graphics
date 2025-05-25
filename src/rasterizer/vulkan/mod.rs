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
        // TODO: or deref device ?
        let device = &self.base.device;

        // TODO: move
        unsafe {
            device
                .wait_for_fences(&[current_frame.fence_render], true, 1_000_000_000)
                .unwrap();
            device.reset_fences(&[current_frame.fence_render]).unwrap();
        }

        // TODO: move create commands
        let (swapchain_img_index, sem_render) = {
            let (swapchain_img_index, image, sem_render) =
                self.swapchain.acquire_next_image(current_frame);

            let cmd_buf_begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            unsafe {
                device
                    .reset_command_buffer(
                        current_frame.cmd_buf,
                        vk::CommandBufferResetFlags::empty(),
                    )
                    .unwrap();
                device
                    .begin_command_buffer(current_frame.cmd_buf, &cmd_buf_begin_info)
                    .unwrap();
            }

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

            unsafe { device.end_command_buffer(current_frame.cmd_buf).unwrap() };

            (swapchain_img_index, sem_render)
        };

        // TODO: move
        {
            let cmd_buf_submit_info =
                [vk::CommandBufferSubmitInfo::default().command_buffer(current_frame.cmd_buf)];
            let wait_semaphore_info = [vk::SemaphoreSubmitInfo::default()
                .semaphore(current_frame.sem_swapchain)
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
                device
                    .queue_submit2(
                        self.commands.queue,
                        &[submit_info],
                        current_frame.fence_render,
                    )
                    .unwrap()
            };
        }

        {
            let swapchains = [self.swapchain.swapchain];
            let wait_semaphores = [*sem_render];
            let images_indices = [swapchain_img_index];
            let present_info = vk::PresentInfoKHR::default()
                .swapchains(&swapchains)
                .wait_semaphores(&wait_semaphores)
                .image_indices(&images_indices);
            assert!(!unsafe {
                self.swapchain
                    .swapchain_loader
                    .queue_present(self.commands.queue, &present_info)
                    .unwrap()
            });
        }

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
