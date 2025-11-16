#[cfg(feature = "vulkan_stats")]
use std::time::Instant;
use std::{
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::vk;
use glam::Mat4;
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

use super::settings::Settings;
use crate::{rasterizer::vulkan::gltf_loader::LoadedGLTF, scene::Camera, window::AppObserver};

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

// TODO: merge with stats + AppObserver ?
#[cfg(feature = "vulkan_stats")]
#[derive(Default, Debug, Clone, Copy)]
struct VulkanStats {
    counts: VulkanStatsCounts,

    resize_micros: u128,
    ui_micros: u128,
    wait_fence_micros: u128,
    compute_shaders_micros: u128,
    scene_update_micros: u128,
    opaque_sort_micros: u128,
    transparent_sort_micros: u128,
    mesh_draw_micros: u128,
    start: VulkanStatsStart,
}

#[cfg(feature = "vulkan_stats")]
#[derive(Default, Debug, Clone, Copy)]
struct VulkanStatsStart {
    base_micros: u128,
    shaders_micros: u128,
    swapchain_micros: u128,
    commands_micros: u128,
    scene_micros: u128,
    gui_micros: u128,
}

#[cfg(feature = "vulkan_stats")]
#[derive(Default, Debug, Clone, Copy)]
pub struct VulkanStatsCounts {
    triangle_count: u32,
    drawcall_count: u32,
    bound_mat: u32,
    bound_mat_pip: u32,
    bound_index_buf: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeshSorting {
    Off,
    Binding,
    Depth,
}

#[derive(Debug, Clone, Copy)]
struct VulkanSettings {
    _validation_layers: bool,
    rebinding: bool,
    opaque_sorting: MeshSorting,
    transparent_sorting: MeshSorting,
    frustum_culling: bool,
}

impl Default for VulkanSettings {
    fn default() -> Self {
        Self {
            _validation_layers: cfg!(feature = "vulkan_validation_layers"),
            rebinding: false,
            // Rebinding is expensive, so we trying to minimize it.
            opaque_sorting: MeshSorting::Binding,
            // Not enough transparent meshes to justify sorting
            transparent_sorting: MeshSorting::Off,
            frustum_culling: true,
        }
    }
}

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
    current_scene: String,

    settings: VulkanSettings,
    #[cfg(feature = "vulkan_stats")]
    stats: VulkanStats,

    selected_node: String,
}

impl Drop for VulkanEngine<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "vulkan_dbg_mem")]
        println!("drop VulkanEngine");
        unsafe {
            self.base.device.device_wait_idle().unwrap();
        }
    }
}

impl VulkanEngine<'_> {
    pub fn new(window: Rc<Window>) -> Self {
        #[cfg(feature = "vulkan_stats")]
        let mut stats = VulkanStats::default();

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        let base = VulkanBase::new(window);
        #[cfg(feature = "vulkan_stats")]
        {
            stats.start.base_micros = t.elapsed().as_micros();
        }

        let allocator = {
            let mut create_info =
                vk_mem::AllocatorCreateInfo::new(&base.instance, &base.device, base.chosen_gpu);
            create_info.flags = vk_mem::AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;
            let allocator = unsafe { vk_mem::Allocator::new(create_info).unwrap() };
            Arc::new(Mutex::new(allocator))
        };

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        let shaders = ShadersLoader::new(base.device.clone());
        #[cfg(feature = "vulkan_stats")]
        {
            stats.start.shaders_micros = t.elapsed().as_micros();
        }
        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        let swapchain = VulkanSwapchain::new(&base, &shaders, allocator.clone(), None);
        #[cfg(feature = "vulkan_stats")]
        {
            stats.start.swapchain_micros = t.elapsed().as_micros();
        }

        let bg_effects_data = swapchain
            .effects
            .bg_effects
            .iter()
            .map(|b| *b.default_data())
            .collect();

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        let commands = VulkanCommands::new(&base, allocator.clone());
        #[cfg(feature = "vulkan_stats")]
        {
            stats.start.commands_micros = t.elapsed().as_micros();
        }

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        let scene = Scene::new(
            &swapchain,
            &commands,
            &shaders,
            base.device.clone(),
            allocator.clone(),
        );
        #[cfg(feature = "vulkan_stats")]
        {
            stats.start.scene_micros = t.elapsed().as_micros();
        }

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        let gui = VulkanGui::new(&base, allocator.clone(), swapchain.swapchain_img_format());
        #[cfg(feature = "vulkan_stats")]
        {
            stats.start.gui_micros = t.elapsed().as_micros();
        }

        Self {
            scene,
            gui,
            commands,
            swapchain,
            shaders,
            allocator,
            base,

            current_bg_effect: 0,
            bg_effects_data,
            current_scene: "basicmesh".into(),

            settings: Default::default(),
            #[cfg(feature = "vulkan_stats")]
            stats,

            selected_node: Default::default(),
        }
    }

    pub fn window(&self) -> &Rc<Window> {
        &self.base.window
    }

    pub fn rasterize(
        &mut self,
        _settings: &Settings,
        camera: &Camera,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] _stats: &mut Stats,
    ) {
        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        self.swapchain.resize_if_necessary(
            &self.base,
            &self.shaders,
            self.commands.allocator.clone(),
        );
        #[cfg(feature = "vulkan_stats")]
        {
            self.stats.resize_micros = t.elapsed().as_micros();
        }

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        let generated_ui = self.gui.generate(|ctx| {
            ui(
                ctx,
                format_debug(
                    app,
                    self.base.window.inner_size(),
                    camera,
                    #[cfg(feature = "vulkan_stats")]
                    self.stats,
                ),
                &mut self.current_bg_effect,
                &mut self.swapchain.render_scale,
                &self.swapchain.effects.bg_effects[..],
                &mut self.bg_effects_data,
                &mut self.current_scene,
                &self.scene.loaded_scenes,
                &mut self.settings,
                &mut self.selected_node,
            )
        });
        #[cfg(feature = "vulkan_stats")]
        {
            self.stats.ui_micros = t.elapsed().as_micros();
        }

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        self.update_scene(camera);
        #[cfg(feature = "vulkan_stats")]
        {
            self.stats.scene_update_micros = t.elapsed().as_micros();
        }

        let image = self.swapchain.draw_img();

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        self.commands.current_frame().wait_for_fences();
        #[cfg(feature = "vulkan_stats")]
        {
            self.stats.wait_fence_micros = t.elapsed().as_micros();
        }

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

        #[cfg(feature = "vulkan_stats")]
        let t = Instant::now();
        current_frame.draw_background(
            &self.swapchain,
            self.current_bg_effect,
            &self.bg_effects_data[self.current_bg_effect],
        );
        #[cfg(feature = "vulkan_stats")]
        {
            self.stats.compute_shaders_micros = t.elapsed().as_micros();
        }

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

        #[cfg(feature = "vulkan_stats")]
        {
            self.stats.counts = Default::default();
        }
        current_frame.draw_geometries(
            &self.settings,
            &self.swapchain,
            &self.scene.view_proj(),
            &self.scene.main_draw_ctx,
            global_desc,
            #[cfg(feature = "vulkan_stats")]
            &mut self.stats,
        );

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
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64), cursor_grabbed: bool) {
        if !cursor_grabbed {
            self.gui.on_mouse_motion(delta);
        }
    }

    fn update_scene(&mut self, camera: &Camera) {
        self.scene.update_scene(
            self.swapchain.draw_extent(),
            camera.view_mat(),
            &self.current_scene,
        );
    }
}

fn ui<'a>(
    ctx: &egui::Context,
    debug: String,
    current_bg_effect: &mut usize,
    render_scale: &mut f32,
    bg_effects: &[ComputeEffect],
    bg_effects_data: &mut [ComputePushConstants],
    current_scene: &mut String,
    scenes: &HashMap<String, LoadedGLTF>,
    settings: &mut VulkanSettings,
    selected_node: &mut String,
) {
    egui::Window::new("Debug")
        .default_open(false)
        .show(ctx, |ui| ui.label(debug));
    egui::Window::new("Settings")
        .default_open(false)
        .show(ctx, |ui| {
            ui.add_enabled(
                false,
                egui::Checkbox::new(
                    &mut cfg!(feature = "vulkan_validation_layers"),
                    "Validation layers",
                ),
            );
            ui.add(egui::Slider::new(render_scale, 0.3..=1.).text("Render scale"));
            ui.checkbox(&mut settings.rebinding, "Rebinding");
            ui.checkbox(&mut settings.frustum_culling, "Frustum culling");
            {
                ui.label("Opaque sorting :");
                ui.radio_value(&mut settings.opaque_sorting, MeshSorting::Off, "off");
                ui.radio_value(
                    &mut settings.opaque_sorting,
                    MeshSorting::Binding,
                    "binding",
                );
                ui.radio_value(&mut settings.opaque_sorting, MeshSorting::Depth, "depth");
            }
            {
                ui.label("Transparent sorting :");
                ui.radio_value(&mut settings.transparent_sorting, MeshSorting::Off, "off");
                ui.radio_value(
                    &mut settings.transparent_sorting,
                    MeshSorting::Binding,
                    "binding",
                );
                ui.radio_value(
                    &mut settings.transparent_sorting,
                    MeshSorting::Depth,
                    "depth",
                );
            }
        });
    egui::Window::new("Background")
        .default_open(false)
        .show(ctx, |ui| {
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
    egui::Window::new("Scene")
        .default_open(false)
        .show(ctx, |ui| {
            ui.label("Selected scene :");
            scenes.keys().for_each(|n| {
                ui.radio_value(current_scene, n.clone(), n);
            });
        });
    egui::Window::new("Scene nodes")
        .default_open(false)
        .show(ctx, |ui| {
            let scene = scenes.get(current_scene).unwrap();
            egui::ScrollArea::vertical()
                .max_height(200.)
                .auto_shrink(true)
                .show(ui, |ui| {
                    scene.nodes.iter().for_each(|(k, _)| {
                        ui.radio_value(selected_node, k.clone(), k);
                    });
                });

            // If the selected node exists (from this scene).
            if let Some(node) = scene.nodes.get(selected_node) {
                let mut node = node.borrow_mut();
                let node_data = node.node_data_mut();
                let local_tr_copy = node_data.local_transform;
                egui::Grid::new("data").num_columns(4).show(ui, |ui| {
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.x_axis.x)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.y_axis.x)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.z_axis.x)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.w_axis.x)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.end_row();
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.x_axis.y)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.y_axis.y)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.z_axis.y)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.w_axis.y)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.end_row();
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.x_axis.z)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.y_axis.z)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.z_axis.z)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.w_axis.z)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.end_row();
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.x_axis.w)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.y_axis.w)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.z_axis.w)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut node_data.local_transform.w_axis.w)
                            .speed(0.01)
                            .range(0.0..=5.0),
                    );
                    ui.end_row();
                });
                // If parameters were modified, update it (and its children) :
                if node_data.local_transform != local_tr_copy {
                    let parent_transform = if let Some(parent) = node_data.parent.upgrade() {
                        parent.borrow().node_data().world_transform
                    } else {
                        Mat4::IDENTITY
                    };
                    node.refresh_transform(&parent_transform);
                }
            }
        });
}

fn format_debug(
    app: &AppObserver,
    size: PhysicalSize<u32>,
    camera: &Camera,
    #[cfg(feature = "vulkan_stats")] stats: VulkanStats,
) -> String {
    #[cfg(feature = "vulkan_stats")]
    let stats = format!("{:#?}", stats);
    #[cfg(not(feature = "vulkan_stats"))]
    let stats = "Stats disabled";
    format!(
        "fps : {} |  r {}μs / f {}μs\nWindow : {}x{}\nCamera : {} p: {} y: {}\n{}",
        app.fps_avg().round(),
        app.last_full_render_loop_micros(),
        app.frame_avg_micros(),
        size.width,
        size.height,
        camera.pos,
        camera.pitch,
        camera.yaw,
        stats,
    )
}
