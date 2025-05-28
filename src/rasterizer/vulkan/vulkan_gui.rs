use std::rc::Rc;

use ash::{Device, vk};
use winit::window::Window;

use super::vulkan_base::VulkanBase;

/// TODO: Immediate : rename from Gui ? or integrate in VulkanCommands ?
pub struct VulkanGui {
    device_copy: Rc<Device>,
}

impl VulkanGui {
    pub fn new(base: &VulkanBase) -> Self {
        let egui_pool = {
            // 1: create descriptor pool for egui
            //  the size of the pool is very oversize, but it's copied from imgui demo
            //  itself.
            let pool_sizes: Vec<vk::DescriptorPoolSize> = vec![
                vk::DescriptorType::SAMPLER,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                vk::DescriptorType::SAMPLED_IMAGE,
                vk::DescriptorType::STORAGE_IMAGE,
                vk::DescriptorType::UNIFORM_TEXEL_BUFFER,
                vk::DescriptorType::STORAGE_TEXEL_BUFFER,
                vk::DescriptorType::UNIFORM_BUFFER,
                vk::DescriptorType::STORAGE_BUFFER,
                vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                vk::DescriptorType::STORAGE_BUFFER_DYNAMIC,
                vk::DescriptorType::INPUT_ATTACHMENT,
            ]
            .drain(..)
            .map(|ty| vk::DescriptorPoolSize {
                ty,
                descriptor_count: 1000,
            })
            .collect();

            let pool_info = vk::DescriptorPoolCreateInfo::default()
                .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
                .max_sets(1000)
                .pool_sizes(&pool_sizes[..]);

            unsafe {
                base.device
                    .create_descriptor_pool(&pool_info, None)
                    .unwrap()
            }
        };

        // 2: initialize egui library
        // this initializes the core structures of egui

        // TODO: set dpi and all ?
        let mut ctx = egui::Context::default();
        let mut info = egui::ViewportInfo::default();
        let viewport_id = dbg!(ctx.viewport_id());
        let native_pixels_per_point = dbg!(ctx.native_pixels_per_point());
        let state = egui_winit::State::new(
            ctx,
            viewport_id,
            &base.window,
            native_pixels_per_point,
            None,
            None,
        );
        // TODO: initialize for winit + vulkan ?
        // base.window

        Self {
            device_copy: base.device.clone(),
        }
    }
}

impl Drop for VulkanGui {
    fn drop(&mut self) {
        println!("drop VulkanGui");
        // unsafe {
        // ?
        // }
    }
}

fn egui(
    ctx: &egui::Context,
    state: &mut egui_winit::State,
    window: &Window,
    viewport_info: &mut egui::ViewportInfo,
) {
    // TODO: call on_window_event + on_mouse_motion ?
    egui_winit::update_viewport_info(viewport_info, ctx, window, false);

    let mut raw_input = state.take_egui_input(window);
    // TODO: multi-viewport handle ?
    raw_input
        .viewports
        .insert(ctx.viewport_id(), viewport_info.clone());

    let full_output = ctx.run(raw_input, |ctx| {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.label("Hello world!");
            if ui.button("Click me").clicked() {
                println!("Click");
            }
        });
    });

    state.handle_platform_output(window, full_output.platform_output);
    let clipped_primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
    // paint(full_output.textures_delta, clipped_primitives);
}
