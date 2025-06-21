use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::vk;
use glam::{Mat4, Quat, Vec3, Vec4, Vec4Swizzles, vec3};
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use super::{format_debug, settings::Settings};
use crate::{scene::World, window::AppObserver};

mod base;
use base::VulkanBase;
mod swapchain;
use swapchain::VulkanSwapchain;
mod commands;
use commands::VulkanCommands;
mod compute_shaders;
use compute_shaders::{ComputeEffect, ComputePushConstants};
mod gui;
use gui::VulkanGui;
mod shaders_loader;
use shaders_loader::ShadersLoader;
mod allocated;
mod descriptors;
mod gfx_pipeline;
mod gltf_loader;
mod scene;
mod textures;
use scene::Scene;

#[cfg(feature = "stats")]
use super::Stats;

/// Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
pub struct VulkanEngine<'a> {
    // Elements are placed in the order they should be dropped, so inverse order of creation.
    scene: Scene<'a>,
    swapchain: VulkanSwapchain,
    gui: VulkanGui,
    commands: VulkanCommands,
    shaders: ShadersLoader,
    allocator: Arc<Mutex<vk_mem::Allocator>>,
    base: VulkanBase,

    current_bg_effect: usize,
    bg_effects_data: Vec<ComputePushConstants>,
    camera: Camera,
}

impl Drop for VulkanEngine<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        println!("drop VulkanEngine");
        unsafe {
            self.base.device.device_wait_idle().unwrap();
        }
    }
}

impl VulkanEngine<'_> {
    pub fn new(window: Rc<Window>) -> Self {
        // panic!("{}", size_of::<vk::DescriptorBufferInfo>());
        let base = VulkanBase::new(window);

        let allocator = {
            let mut create_info =
                vk_mem::AllocatorCreateInfo::new(&base.instance, &base.device, base.chosen_gpu);
            create_info.flags = vk_mem::AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;
            let allocator = unsafe { vk_mem::Allocator::new(create_info).unwrap() };
            Arc::new(Mutex::new(allocator))
        };

        let shaders = ShadersLoader::new(base.device.clone());
        let swapchain = VulkanSwapchain::new(&base, &shaders, allocator.clone(), None);

        let bg_effects_data = swapchain
            .effects
            .bg_effects
            .iter()
            .map(|b| *b.default_data())
            .collect();

        let commands = VulkanCommands::new(&base, allocator.clone());

        let scene = Scene::new(
            &swapchain,
            &commands,
            &shaders,
            base.device.clone(),
            allocator.clone(),
        );

        Self {
            scene,
            gui: VulkanGui::new(&base, allocator.clone(), swapchain.swapchain_img_format()),
            commands,
            swapchain,
            shaders,
            allocator,
            base,

            current_bg_effect: 0,
            bg_effects_data,
            camera: Default::default(),
        }
    }

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

        let generated_ui = self.gui.generate(|ctx| {
            ui(
                ctx,
                format_debug(settings, world, app, self.base.window.inner_size(), None),
                &mut self.current_bg_effect,
                &mut self.swapchain.render_scale,
                &self.swapchain.effects.bg_effects[..],
                &mut self.bg_effects_data,
            )
        });

        self.update_scene();

        let image = self.swapchain.draw_img();

        self.commands.current_frame().wait_for_fences();

        let current_frame = self.commands.current_frame_mut();
        current_frame.clear_descriptors();
        current_frame.clear_buffers_in_use();
        let global_desc = current_frame
            .descriptors_mut()
            .allocate(self.scene.data_descriptor_layout);

        let current_frame = self.commands.current_frame();

        let Some((swapchain_img_index, swapchain_image, sem_render, swapchain_image_view)) =
            self.swapchain.acquire_next_image(current_frame)
        else {
            return;
        };

        current_frame.reset_fences();
        current_frame.begin_cmd_buf();

        current_frame.transition_image(
            *image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );

        current_frame.draw_background(
            &self.swapchain,
            self.current_bg_effect,
            &self.bg_effects_data[self.current_bg_effect],
        );

        current_frame.transition_image(
            *image,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );
        let depth = self.swapchain.depth_img();
        current_frame.transition_image(
            *depth,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
        );

        let buffer_in_use =
            self.scene
                .upload_data(&self.base.device, self.allocator.clone(), global_desc);

        current_frame.draw_geometries(&self.swapchain, &self.scene.main_draw_ctx, global_desc);

        current_frame.transition_image(
            *image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );
        current_frame.transition_image(
            *swapchain_image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );

        current_frame.copy_img(
            *image,
            *swapchain_image,
            self.swapchain.draw_extent(),
            self.swapchain.swapchain_extent(),
        );

        current_frame.transition_image(
            *swapchain_image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );

        current_frame.draw_gui(
            &self.swapchain,
            &self.gui,
            self.commands.queue,
            *swapchain_image_view,
            generated_ui,
        );

        current_frame.transition_image(
            *swapchain_image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        );
        current_frame.end_cmd_buf();
        current_frame.submit(sem_render, self.commands.queue);
        self.swapchain
            .present(swapchain_img_index, sem_render, self.commands.queue);

        self.commands
            .current_frame_mut()
            .push_buffer_in_use(buffer_in_use);
        self.commands.frame_number += 1;
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.gui.on_window_event(event);
        self.camera.on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64), cursor_grabbed: bool) {
        self.gui.on_mouse_motion(delta);
        self.camera.on_mouse_motion(delta, cursor_grabbed);
    }

    fn update_scene(&mut self) {
        self.camera.update();
        self.scene
            .update_scene(self.swapchain.draw_extent(), self.camera.view_mat());
    }
}

fn ui(
    ctx: &egui::Context,
    debug: String,
    current_bg_effect: &mut usize,
    render_scale: &mut f32,
    bg_effects: &[ComputeEffect],
    bg_effects_data: &mut [ComputePushConstants],
) {
    egui::Window::new("debug").show(ctx, |ui| ui.label(debug));
    egui::Window::new("Background").show(ctx, |ui| {
        ui.add(egui::Slider::new(render_scale, 0.3..=1.).text("Render scale"));
        if !bg_effects.is_empty() {
            ui.label("Selected effect :");
            bg_effects.iter().enumerate().for_each(|(i, n)| {
                ui.radio_value(current_bg_effect, i, n.name.into_str());
            });

            let current_bg_effect_data = &mut bg_effects_data[*current_bg_effect];
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
                    ui.add(egui::DragValue::new(d).speed(0.01).range(0.0..=1.0));
                });
            });
        }
    });
}

// TODO: merge camera with general engine
struct Camera {
    vel: Vec3,
    pitch: f32,
    yaw: f32,

    pos: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            vel: Default::default(),
            pitch: Default::default(),
            yaw: Default::default(),

            // pos: vec3(0., 0., 5.),
            pos: vec3(30., -0., -85.),
        }
    }
}

impl Camera {
    fn view_mat(&self) -> Mat4 {
        // to create a correct model view, we need to move the world in opposite
        // direction to the camera
        //  so we will create the camera model matrix and invert
        let tr = Mat4::from_translation(self.pos);
        let rot = self.rot_mat();
        (tr * rot).inverse()
    }

    fn rot_mat(&self) -> Mat4 {
        // fairly typical FPS style camera. we join the pitch and yaw rotations into
        // the final rotation matrix
        let pitch = Quat::from_axis_angle(vec3(1., 0., 0.), self.pitch);
        let yaw = Quat::from_axis_angle(vec3(0., -1., 0.), self.yaw);

        Mat4::from_quat(yaw * pitch)
    }

    // TODO: doesn't take into account time delta.
    fn update(&mut self) {
        let rot = self.rot_mat();
        self.pos += (rot * Vec4::from((self.vel * 0.5, 0.))).xyz();
    }

    fn on_window_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state,
                        ..
                    },
                ..
            } = event { match state {
            ElementState::Pressed => match key {
                KeyCode::KeyW => self.vel.z = -1.,
                KeyCode::KeyS => self.vel.z = 1.,
                KeyCode::KeyA => self.vel.x = -1.,
                KeyCode::KeyD => self.vel.x = 1.,
                _ => (),
            },
            ElementState::Released => match key {
                KeyCode::KeyW => self.vel.z = 0.,
                KeyCode::KeyS => self.vel.z = 0.,
                KeyCode::KeyA => self.vel.x = 0.,
                KeyCode::KeyD => self.vel.x = 0.,
                _ => (),
            },
        } }
    }

    fn on_mouse_motion(&mut self, (delta_x, delta_y): (f64, f64), cursor_grabbed: bool) {
        if cursor_grabbed {
            self.yaw += delta_x as f32 / 200.;
            self.pitch -= delta_y as f32 / 200.;
        }
    }
}
