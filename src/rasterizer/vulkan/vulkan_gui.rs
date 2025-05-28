use std::rc::Rc;

use ash::{Device, vk};
use winit::{event::WindowEvent, window::Window};

use super::vulkan_base::VulkanBase;

pub struct VulkanGui {
    device_copy: Rc<Device>,
    window: Rc<Window>,
    pool: vk::DescriptorPool,
    state: egui_winit::State,
    info: egui::ViewportInfo,
}

impl VulkanGui {
    pub fn new(base: &VulkanBase) -> Self {
        let pool = {
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

        let ctx = egui::Context::default();
        let info = egui::ViewportInfo::default();
        let viewport_id = ctx.viewport_id();
        let state = egui_winit::State::new(
            ctx,
            viewport_id,
            &base.window,
            Some(base.window.scale_factor() as f32),
            None,
            None, // TODO: max_texture_side, useful ?
        );
        // TODO: initialize for vulkan ?
        // base.window

        Self {
            device_copy: base.device.clone(),
            window: base.window.clone(),
            pool,
            state,
            info,
        }
    }

    pub fn draw(&mut self) {
        // TODO: call on_window_event + on_mouse_motion ?
        egui_winit::update_viewport_info(
            &mut self.info,
            self.state.egui_ctx(),
            &self.window,
            false,
        );

        let raw_input = self.state.take_egui_input(&self.window);
        // Already filled, but in the docs it says it doesn't...
        // self.state
        //     .egui_ctx()
        //     .input(|i| raw_input.viewports = i.raw.viewports.clone());

        let full_output = self.state.egui_ctx().run(raw_input, |ctx| {
            egui::CentralPanel::default().show(&ctx, |ui| {
                ui.label("Hello world!");
                if ui.button("Click me").clicked() {
                    println!("Click");
                }
            });
        });

        self.state
            .handle_platform_output(&self.window, full_output.platform_output);
        let clipped_primitives = self
            .state
            .egui_ctx()
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        // paint(full_output.textures_delta, clipped_primitives);
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        // TODO: result ?
        let _ = self.state.on_window_event(&self.window, event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.state.on_mouse_motion(delta);
    }
}

impl Drop for VulkanGui {
    fn drop(&mut self) {
        println!("drop VulkanGui");
        unsafe {
            self.device_copy.destroy_descriptor_pool(self.pool, None);
        }
    }
}
