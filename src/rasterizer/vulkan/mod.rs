use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    mem,
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::vk;
use glam::{Mat4, Vec3, Vec4, vec3, vec4};
use winit::{event::WindowEvent, window::Window};

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
mod gfx_pipeline;
use gfx_pipeline::VkGraphicsPipeline;
mod descriptors;
use descriptors::{DescriptorAllocatorGrowable, DescriptorLayoutBuilder, DescriptorWriter};
mod allocated;
use allocated::{AllocatedBuffer, MyMemoryUsage};
mod gltf_loader;
mod textures;
use textures::Textures;
mod scene;
use scene::{DrawContext, GltfMetallicRoughness, GpuSceneData, MeshNode, Node};

#[cfg(feature = "stats")]
use super::Stats;

/// Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
pub struct VulkanEngine<'a> {
    // Elements are placed in the order they should be dropped, so inverse order of creation.

    // TODO: move to gfx
    gpu_scene_data_buffer: Vec<AllocatedBuffer>,
    main_draw_ctx: DrawContext,
    loaded_nodes: HashMap<String, Rc<RefCell<dyn Node>>>,
    // TODO: move to textures ?
    _metal_rough_material: GltfMetallicRoughness<'a>,
    // TODO: move to textures
    /// Keep it to prevent de-allocation
    _material_constants: AllocatedBuffer,
    _global_desc_alloc: DescriptorAllocatorGrowable,
    gpu_scene_data_descriptor_layout: vk::DescriptorSetLayout,

    _textures: Textures,
    gfx: VkGraphicsPipeline,
    swapchain: VulkanSwapchain,
    gui: VulkanGui,
    commands: VulkanCommands,
    shaders: ShadersLoader,
    allocator: Arc<Mutex<vk_mem::Allocator>>,
    base: VulkanBase,

    current_bg_effect: usize,
    bg_effects_data: Vec<ComputePushConstants>,

    scene_data: GpuSceneData,
}

impl Drop for VulkanEngine<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        println!("drop VulkanEngine");
        unsafe {
            self.base.device.device_wait_idle().unwrap();
            self.base
                .device
                .destroy_descriptor_set_layout(self.gpu_scene_data_descriptor_layout, None);
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

        let textures = Textures::new(&commands, base.device.clone(), allocator.clone());

        let gpu_scene_data_descriptor_layout = DescriptorLayoutBuilder::default()
            .add_binding(0, vk::DescriptorType::UNIFORM_BUFFER)
            .build(
                &base.device,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            );
        let mut global_desc_alloc = DescriptorAllocatorGrowable::new_global(base.device.clone());

        let mut metal_rough_material = GltfMetallicRoughness::new(
            base.device.clone(),
            &shaders,
            *swapchain.draw_format(),
            *swapchain.depth_format(),
            gpu_scene_data_descriptor_layout,
        );

        let (material_constants, default_data) = metal_rough_material.create_material(
            allocator.clone(),
            &textures,
            &mut global_desc_alloc,
        );

        let default_data = Rc::new(default_data);

        let mut gfx = VkGraphicsPipeline::new(
            &commands,
            &shaders,
            default_data.clone(),
            base.device.clone(),
            *swapchain.draw_format(),
            *swapchain.depth_format(),
        );

        let loaded_nodes = gfx
            .meshes
            .drain(..)
            .map(|m| {
                let name = m.name.clone();
                let v: Rc<RefCell<dyn Node>> = Rc::new(RefCell::new(MeshNode::new(
                    m,
                    Mat4::IDENTITY,
                    Mat4::IDENTITY,
                )));
                (name, v)
            })
            .collect();

        Self {
            gpu_scene_data_buffer: Default::default(),
            main_draw_ctx: Default::default(),
            loaded_nodes,
            _metal_rough_material: metal_rough_material,
            _material_constants: material_constants,
            _global_desc_alloc: global_desc_alloc,
            gpu_scene_data_descriptor_layout,
            _textures: textures,
            gfx,
            gui: VulkanGui::new(&base, allocator.clone(), swapchain.swapchain_img_format()),
            commands,
            swapchain,
            shaders,
            base,

            current_bg_effect: 0,
            bg_effects_data,

            allocator,

            scene_data: Default::default(),
        }
    }

    pub fn window(&self) -> &Rc<Window> {
        &self.base.window
    }

    pub fn update_scene(&mut self) {
        self.main_draw_ctx.clear();

        self.loaded_nodes
            .get("Suzanne")
            .unwrap()
            .borrow()
            .draw(&Mat4::IDENTITY, &mut self.main_draw_ctx);

        {
            let cube = self.loaded_nodes["Cube"].borrow();
            (-3..3).for_each(|x| {
                let scale = Mat4::from_scale(Vec3::splat(0.2));
                let translation = Mat4::from_translation(vec3(x as f32, 1., 0.));

                cube.draw(&(translation * scale), &mut self.main_draw_ctx);
            });
        }

        let view = Mat4::from_translation(vec3(0., 0., -5.));
        // Camera projection
        let draw_extent = self.swapchain.draw_extent();
        let mut proj = Mat4::perspective_rh(
            70.,
            draw_extent.width as f32 / draw_extent.height as f32,
            10_000.,
            0.1,
        );
        proj.y_axis[1] *= -1.;
        // TODO: move to setters ?
        self.scene_data = GpuSceneData {
            view,
            proj,
            view_proj: self.scene_data.proj * self.scene_data.view,
            ambient_color: Vec4::splat(1.),
            sunlight_direction: vec4(0., 1., 0.5, 1.),
            sunlight_color: Vec4::splat(1.),
        };
    }

    // TODO: move all this code to separate rendering pipeline file
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
        let global_desc = current_frame
            .descriptors_mut()
            .allocate(self.gpu_scene_data_descriptor_layout);

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

        // TODO: move
        // TODO: store until next frame so GPU has time to use it ?
        {
            // We will also dynamically allocate the uniform buffer itself as a way to
            // showcase how you could do temporal per-frame data that is dynamically created.
            // It would be better to hold the buffers cached in our FrameData structure,
            // but we will be doing it this way to show how.
            // There are cases with dynamic draws and passes where you might want to do it
            // this way.
            let gpu_scene_data_buffer = AllocatedBuffer::new(
                self.allocator.clone(),
                size_of::<GpuSceneData>() as u64,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                MyMemoryUsage::CpuToGpu,
            );
            let scene_data = unsafe {
                mem::transmute::<*mut c_void, &mut GpuSceneData>(
                    gpu_scene_data_buffer.mapped_data(),
                )
            };
            *scene_data = self.scene_data;

            let mut writer = DescriptorWriter::default();
            writer.write_buffer(
                0,
                gpu_scene_data_buffer.buffer,
                size_of::<GpuSceneData>() as u64,
                0,
                vk::DescriptorType::UNIFORM_BUFFER,
            );
            writer.update_set(&self.base.device, global_desc);

            self.gpu_scene_data_buffer.push(gpu_scene_data_buffer);
        }

        current_frame.draw_geometries(&self.swapchain, &self.gfx, &self.main_draw_ctx, global_desc);

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

        self.commands.frame_number += 1;
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.gui.on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.gui.on_mouse_motion(delta);
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
