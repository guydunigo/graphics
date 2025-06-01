use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::vk;
use vulkan_descriptors::ComputeEffect;
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
use vulkan_gui::{GeneratedUi, VulkanGui};
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

    current_bg_effect: usize,
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
        let generated_ui = self.gui.generate(|ctx| {
            ui(
                ctx,
                format_debug(settings, world, app, self.base.window.inner_size(), None),
                &mut self.current_bg_effect,
                &mut self.swapchain.descriptors.bg_effects[..],
            )
        });

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

        current_frame.draw_background(&self.swapchain, self.current_bg_effect);

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

        self.draw_gui(
            current_frame.cmd_pool,
            current_frame.cmd_buf,
            *swapchain_image_view,
            generated_ui,
        );

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

        // todo!("move bg_effects state out of swapchain (resize)");
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

            current_bg_effect: 0,
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.gui.on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.gui.on_mouse_motion(delta);
    }

    fn draw_gui(
        &self,
        cmd_pool: vk::CommandPool,
        cmd_buf: vk::CommandBuffer,
        target_img_view: vk::ImageView,
        generated_ui: GeneratedUi,
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
            generated_ui,
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

fn ui(
    ctx: &egui::Context,
    debug: String,
    current_bg_effect: &mut usize,
    bg_effects: &mut [ComputeEffect],
) {
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
    egui::Window::new("Background").show(&ctx, |ui| {
        if bg_effects.len() > 0 {
            ui.label("Selected effect :");
            bg_effects.iter().enumerate().for_each(|(i, n)| {
                ui.radio_value(current_bg_effect, i, n.name.into_str());
            });

            let current_bg_effect_data = &mut bg_effects[*current_bg_effect].data;
            egui::Grid::new("data").num_columns(5).show(ui, |ui| {
                ui.label("Data 0");
                current_bg_effect_data.data0.iter_mut().for_each(|d| {
                    ui.add(egui::DragValue::new(d).speed(0.01).range(0.0..=1.0));
                });
                ui.end_row();

                ui.label("Data 1");
                current_bg_effect_data.data1.iter_mut().for_each(|d| {
                    ui.add(egui::DragValue::new(d).speed(0.01).range(0.0..=1.0));
                });
                ui.end_row();

                ui.label("Data 2");
                current_bg_effect_data.data2.iter_mut().for_each(|d| {
                    ui.add(egui::DragValue::new(d).speed(0.01).range(0.0..=1.0));
                });
                ui.end_row();

                ui.label("Data 3");
                current_bg_effect_data.data3.iter_mut().for_each(|d| {
                    ui.add(egui::DragValue::new(d));
                });
            });
        }
    });
}

// Converts rgba a u32 (4*[0,255]) to (4*[0.,1.])
// fn rgba_u32_to_f32(color: egui::Color32) -> [f32; 4] {
//     egui::Rgba::from(color).to_array()
// }
