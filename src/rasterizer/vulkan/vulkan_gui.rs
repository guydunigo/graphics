use std::rc::Rc;

use ash::{Device, vk};

use super::vulkan_base::VulkanBase;

/// TODO: Immediate : rename from Gui ? or integrate in VulkanCommands ?
pub struct VulkanGui {
    device_copy: Rc<Device>,
}

impl VulkanGui {
    pub fn new(base: &VulkanBase) -> Self {
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

fn egui() {
    // set dpi and all
    let mut ctx = egui::Context::default();

    // TODO: What has happened (cursor pos and keys ?)
    // https://docs.rs/egui/latest/egui/struct.RawInput.html
    // or use egui-winit
    let raw_input: egui::RawInput = egui::RawInput::default();

    let full_output = ctx.run(raw_input, |ctx| {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.label("Hello world!");
            if ui.button("Click me").clicked() {
                println!("Click");
            }
        });
    });

    // TODO: handle cursor change, copy events, ...
    // https://docs.rs/egui/latest/egui/struct.FullOutput.html
    // handle_platform_output(full_output.platform_output);
    let clipped_primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
    // paint(full_output.textures_delta, clipped_primitives);
}
