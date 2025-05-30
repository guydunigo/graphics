use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::vk;
use winit::{event::WindowEvent, window::Window};

use crate::{scene::World, window::AppObserver};

use super::{format_debug, settings::Settings};

mod vulkan_base;
use vulkan_base::VulkanBase;
mod vulkan_swapchain;
use vulkan_swapchain::VulkanSwapchain;
mod vulkan_commands;
use vulkan_commands::VulkanCommands;
mod vulkan_descriptors;
mod vulkan_gui;
use vulkan_gui::VulkanGui;
mod vulkan_shaders;
use vulkan_shaders::VulkanShaders;

#[cfg(feature = "stats")]
use super::Stats;

/// Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
pub struct VulkanEngine {
    // Elements are placed in the order they should be dropped, so inverse order of creation.
    swapchain: VulkanSwapchain,
    gui: VulkanGui,
    commands: VulkanCommands,
    shaders: VulkanShaders,
    base: VulkanBase,
    // TODO: window_extent: vk::Extent2D,
}

impl VulkanEngine {
    pub fn window(&self) -> &Rc<Window> {
        &self.base.window
    }

    pub fn rasterize(
        &mut self,
        settings: &Settings,
        world: &World,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] _stats: &mut Stats,
    ) {
        self.swapchain.resize_if_necessary(
            &self.base,
            &self.shaders,
            self.commands.allocator.clone(),
        );

        let current_frame = self.commands.current_frame();
        let image = self.swapchain.draw_img();

        current_frame.wait_for_fences();
        current_frame.begin_cmd_buf();
        current_frame.transition_image(
            *image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );

        current_frame.draw_background(&self.swapchain);

        let (swapchain_img_index, swapchain_image, sem_render, swapchain_image_view) =
            self.swapchain.acquire_next_image(current_frame);
        current_frame.copy_img_swapchain(
            *image,
            self.swapchain.draw_extent(),
            *swapchain_image,
            self.swapchain.swapchain_extent,
        );

        current_frame.transition_image(
            *swapchain_image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );

        // Releasing `current_frame` and other references on self, to get a mut for egui.
        let sem_render = *sem_render;
        let swapchain_image = *swapchain_image;
        {
            let cmd_pool = current_frame.cmd_pool;
            let cmd_buf = current_frame.cmd_buf;
            let debug = format_debug(settings, world, app, self.base.window.inner_size(), None);
            todo!("refcell to clean this mess");
            self.draw_imgui(cmd_pool, cmd_buf, *swapchain_image_view, debug);
        }

        let current_frame = self.commands.current_frame();

        current_frame.transition_image(
            swapchain_image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        );
        current_frame.end_cmd_buf();
        current_frame.submit(&sem_render, self.commands.queue);
        self.swapchain
            .present(swapchain_img_index, &sem_render, self.commands.queue);

        self.commands.frame_number += 1;
    }

    pub fn new(window: Rc<Window>) -> Self {
        let base = VulkanBase::new(window);

        let allocator = {
            let mut create_info =
                vk_mem::AllocatorCreateInfo::new(&base.instance, &base.device, base.chosen_gpu);
            create_info.flags = vk_mem::AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;
            let allocator = unsafe { vk_mem::Allocator::new(create_info).unwrap() };
            Arc::new(Mutex::new(allocator))
        };

        let shaders = VulkanShaders::new(base.device.clone());
        let swapchain = VulkanSwapchain::new(&base, &shaders, allocator.clone());
        Self {
            gui: VulkanGui::new(&base, allocator.clone(), swapchain.swapchain_img_format),
            commands: VulkanCommands::new(&base, allocator),
            swapchain,
            shaders,
            base,
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.gui.on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.gui.on_mouse_motion(delta);
    }

    fn draw_imgui(
        &mut self,
        cmd_pool: vk::CommandPool,
        cmd_buf: vk::CommandBuffer,
        target_img_view: vk::ImageView,
        debug: String,
    ) {
        let color_attachments = [attachment_info(
            target_img_view,
            None,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        )];
        let render_info = rendering_info(
            self.swapchain.swapchain_extent,
            &color_attachments[..],
            None,
        );

        unsafe {
            self.base.device.cmd_begin_rendering(cmd_buf, &render_info);
        }

        self.gui.draw(
            self.commands.queue,
            self.swapchain.draw_extent(),
            cmd_pool,
            cmd_buf,
            debug,
        );

        unsafe {
            self.base.device.cmd_end_rendering(cmd_buf);
        }
    }
}

fn attachment_info<'a>(
    view: vk::ImageView,
    clear: Option<vk::ClearValue>,
    layout: vk::ImageLayout,
) -> vk::RenderingAttachmentInfo<'a> {
    let load_op = clear
        .map(|_| vk::AttachmentLoadOp::CLEAR)
        .unwrap_or(vk::AttachmentLoadOp::LOAD);
    let mut res = vk::RenderingAttachmentInfo::default()
        .image_view(view)
        .image_layout(layout)
        .load_op(load_op)
        .store_op(vk::AttachmentStoreOp::STORE);

    if let Some(clear) = clear {
        res.clear_value = clear;
    }

    res
}

fn rendering_info<'a>(
    extent: vk::Extent2D,
    color_attachments: &'a [vk::RenderingAttachmentInfo],
    depth_attachment: Option<&'a vk::RenderingAttachmentInfo>,
) -> vk::RenderingInfo<'a> {
    let res = vk::RenderingInfo::default()
        .render_area(vk::Rect2D {
            offset: Default::default(),
            extent,
        })
        .layer_count(1)
        .color_attachments(color_attachments);

    if let Some(depth_attachment) = depth_attachment {
        res.depth_attachment(depth_attachment)
    } else {
        res
    }
}
