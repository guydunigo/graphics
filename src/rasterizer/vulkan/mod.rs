use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::vk;
use egui::Color32;
use vulkan_descriptors::ComputePushConstants;
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

    color_top: Color32,
    color_bot: Color32,
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

        {
            let constants = ComputePushConstants {
                data0: egui::Rgba::from(self.color_top).to_array(),
                data1: egui::Rgba::from(self.color_bot).to_array(),
                ..Default::default()
            };
            current_frame.draw_background(&self.swapchain, &constants);
        }

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

        {
            let debug = format_debug(settings, world, app, self.base.window.inner_size(), None);
            let mut color_top = self.color_top;
            let color_top_ref = &mut color_top;
            let mut color_bot = self.color_bot;
            let color_bot_ref = &mut color_bot;
            self.draw_imgui(
                current_frame.cmd_pool,
                current_frame.cmd_buf,
                *swapchain_image_view,
                move |ctx| ui(ctx, debug.as_str(), color_top_ref, color_bot_ref),
            );
            self.color_top = color_top;
            self.color_bot = color_bot;
        };

        current_frame.transition_image(
            *swapchain_image,
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

            color_top: Color32::from_rgba_unmultiplied(255, 0, 0, 255),
            color_bot: Color32::from_rgba_unmultiplied(0, 0, 255, 125),
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.gui.on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.gui.on_mouse_motion(delta);
    }

    fn draw_imgui(
        &self,
        cmd_pool: vk::CommandPool,
        cmd_buf: vk::CommandBuffer,
        target_img_view: vk::ImageView,
        ui: impl FnMut(&egui::Context),
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
            ui,
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

fn ui(ctx: &egui::Context, debug: &str, color_top: &mut Color32, color_bot: &mut Color32) {
    egui::Window::new("debug").show(&ctx, |ui| ui.label(debug));
    egui::Window::new("test").show(&ctx, |ui| {
        ui.label("Hello world!");
        if ui.button("Click me").clicked() {
            println!("Click");
        }
        ui.heading("My Heading is big !!!");
        ui.menu_button("My menu", |ui| {
            ui.menu_button("My sub-menu", |ui| {
                if ui.button("Close the menu").clicked() {
                    ui.close_menu();
                }
            });
        });
    });
    egui::Window::new("Color pickers").show(&ctx, |ui| {
        // ui.columns_const(|[c0, c1]| {
        //     c0.label("Top color :");
        //     c1.color_edit_button_srgba(color_top);
        // });
        // ui.columns_const(|[c0, c1]| {
        //     c0.label("Bottom color :");
        //     c1.color_edit_button_srgba(color_bot);
        // });
        ui.label("Top color :");
        ui.color_edit_button_srgba(color_top);
        ui.label("Bottom color :");
        ui.color_edit_button_srgba(color_bot);
    });
}
