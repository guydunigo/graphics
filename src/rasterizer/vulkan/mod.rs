use std::{
    ffi::c_void,
    mem,
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

use ash::{Device, vk};
use glam::{Mat4, Vec4};
use vk_mem::Allocator;
use winit::{event::WindowEvent, window::Window};

use super::{format_debug, settings::Settings};
use crate::{scene::World, window::AppObserver};

mod base;
use base::VulkanBase;
mod swapchain;
use swapchain::{AllocatedImage, VulkanSwapchain};
mod commands;
use commands::VulkanCommands;
mod compute_shaders;
use compute_shaders::{ComputeEffect, ComputePushConstants};
mod gui;
use gui::VulkanGui;
mod shaders_loader;
use shaders_loader::{ShaderName, ShadersLoader};
mod gfx_pipeline;
use gfx_pipeline::{GpuDrawPushConstants, PipelineBuilder, VkGraphicsPipeline};
mod descriptors;
use descriptors::{
    AllocatedBuffer, DescriptorAllocatorGrowable, DescriptorLayoutBuilder, DescriptorWriter,
    MyMemoryUsage,
};
mod gltf_loader;
mod textures;
use textures::Textures;

#[cfg(feature = "stats")]
use super::Stats;

/// Inspired from vkguide.dev and ash-examples/src/lib.rs since we don't have VkBootstrap
pub struct VulkanEngine<'a> {
    // TODO: keep here ?
    default_data: MaterialInstance,
    metal_rough_material: GltfMetallicRoughness<'a>,
    _material_constants: AllocatedBuffer,
    global_desc_alloc: DescriptorAllocatorGrowable,
    gpu_scene_data_descriptor_layout: vk::DescriptorSetLayout,

    // Elements are placed in the order they should be dropped, so inverse order of creation.
    textures: Textures,
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

        Self {
            default_data,
            metal_rough_material,
            _material_constants: material_constants,
            global_desc_alloc,
            gpu_scene_data_descriptor_layout,
            textures,
            gfx: VkGraphicsPipeline::new(
                &commands,
                &shaders,
                base.device.clone(),
                *swapchain.draw_format(),
                *swapchain.depth_format(),
            ),
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

        let image = self.swapchain.draw_img();

        self.commands.current_frame().wait_for_fences();
        self.commands.current_frame_mut().clear_descriptors();

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

        current_frame.draw_geometries(&self.swapchain, &self.gfx, &self.textures);

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

        // TODO: move
        // TODO: store until next frame so GPU has time to use it
        if false {
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

            let global_descriptor = self
                .commands
                .current_frame_mut()
                .descriptors_mut()
                .allocate(self.gpu_scene_data_descriptor_layout);

            let mut writer = DescriptorWriter::default();
            writer.write_buffer(
                0,
                gpu_scene_data_buffer.buffer,
                size_of::<GpuSceneData>() as u64,
                0,
                vk::DescriptorType::UNIFORM_BUFFER,
            );
            writer.update_set(&self.base.device, global_descriptor);
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        self.gui.on_window_event(event);
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        self.gui.on_mouse_motion(delta);
    }
}

// Converts rgba a u32 (4*[0,255]) to (4*[0.,1.])
// fn rgba_u32_to_f32(color: egui::Color32) -> [f32; 4] {
//     egui::Rgba::from(color).to_array()
// }

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

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
struct GpuSceneData {
    view: Mat4,
    proj: Mat4,
    view_proj: Mat4,
    ambient_color: Vec4,
    sunlight_direction: Vec4,
    sunlight_color: Vec4,
}

// TODO: move these
struct RenderObject<'a> {
    // TODO: or slice/range ?
    index_count: usize,
    first_index: usize,
    index_buffer: vk::Buffer,

    material: &'a MaterialInstance,

    transform: Mat4,
    vertex_buffer_addr: vk::DeviceAddress,
}

/// The fields are supposed to be destroyed by the parent class GltfMetallicRoughness
struct MaterialPipeline {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}

enum MaterialPass {
    MainColor,
    Transparent,
    Other,
}

struct MaterialInstance {
    pipeline: Weak<MaterialPipeline>,
    material_set: vk::DescriptorSet,
    pass_type: MaterialPass,
}

// TODO: move
// trait Renderable {
//     fn draw(top_mat: &Mat4, ctx: &DrawContext);
// }

#[repr(C)]
struct MaterialConstants {
    color_factors: Vec4,
    metal_rough_factors: Vec4,
    // padding up to 256 bit alignment, we need it anyway for uniform buffer
    // TODO needed in rust ??? what if we use utils' Align copy ?
    // extra: [Vec4; 14],
}

struct MaterialResources<'a> {
    color_img: &'a AllocatedImage,
    color_sampler: vk::Sampler,
    metal_rough_img: &'a AllocatedImage,
    metal_rough_sampler: vk::Sampler,
    data_buffer: vk::Buffer,
    data_buffer_offset: u32,
}

/// This struct should be the master of the pipelines and layouts,
/// it takes care of destroying the layout on drop.
struct GltfMetallicRoughness<'a> {
    device_copy: Rc<Device>,

    pipeline_opaque: Rc<MaterialPipeline>,
    pipeline_transparent: Rc<MaterialPipeline>,

    material_layout: vk::DescriptorSetLayout,

    writer: DescriptorWriter<'a>,
}

impl Drop for GltfMetallicRoughness<'_> {
    fn drop(&mut self) {
        println!("drop GltfMetallicRoughness");
        unsafe {
            self.device_copy
                .destroy_descriptor_set_layout(self.material_layout, None);

            self.device_copy
                .destroy_pipeline_layout(self.pipeline_opaque.layout, None);
            if self.pipeline_opaque.layout != self.pipeline_transparent.layout {
                self.device_copy
                    .destroy_pipeline_layout(self.pipeline_transparent.layout, None);
            }

            self.device_copy
                .destroy_pipeline(self.pipeline_opaque.pipeline, None);
            self.device_copy
                .destroy_pipeline(self.pipeline_transparent.pipeline, None);
        }
    }
}

impl GltfMetallicRoughness<'_> {
    fn new(
        device: Rc<Device>,
        shaders: &ShadersLoader,
        draw_img_format: vk::Format,
        depth_img_format: vk::Format,
        gpu_scene_data_descriptor_layout: vk::DescriptorSetLayout,
    ) -> Self {
        let mesh_frag = shaders.get(ShaderName::MeshFrag);
        let mesh_vert = shaders.get(ShaderName::MeshVert);

        let matrix_range = vk::PushConstantRange::default()
            .offset(0)
            .size(size_of::<GpuDrawPushConstants>() as u32)
            .stage_flags(vk::ShaderStageFlags::VERTEX);
        let push_constant_ranges = [matrix_range];

        let material_layout = DescriptorLayoutBuilder::default()
            .add_binding(0, vk::DescriptorType::UNIFORM_BUFFER)
            .add_binding(1, vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .add_binding(2, vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .build(
                &device,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            );
        let layouts = [gpu_scene_data_descriptor_layout, material_layout];

        let mesh_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts[..])
            .push_constant_ranges(&push_constant_ranges[..]);

        let new_layout = unsafe {
            device
                .create_pipeline_layout(&mesh_layout_info, None)
                .unwrap()
        };

        let mut pipeline_builder = PipelineBuilder::new(new_layout);
        pipeline_builder.set_shaders(&mesh_vert, &mesh_frag);
        pipeline_builder.set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        pipeline_builder.set_polygon_mode(vk::PolygonMode::FILL);
        pipeline_builder.set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE);
        pipeline_builder.set_multisampling_none();
        pipeline_builder.disable_blending();
        pipeline_builder.enable_depthtest(true, vk::CompareOp::GREATER_OR_EQUAL);
        // render format
        let formats = [draw_img_format];
        pipeline_builder.set_color_attachment_format(&formats[..]);
        pipeline_builder.set_depth_format(depth_img_format);

        let pipeline_opaque = pipeline_builder.build(&device);

        pipeline_builder.enable_blending_additive();
        pipeline_builder.enable_depthtest(false, vk::CompareOp::GREATER_OR_EQUAL);
        let pipeline_transparent = pipeline_builder.build(&device);

        Self {
            device_copy: device,

            pipeline_opaque: Rc::new(MaterialPipeline {
                pipeline: pipeline_opaque,
                layout: new_layout,
            }),
            pipeline_transparent: Rc::new(MaterialPipeline {
                pipeline: pipeline_transparent,
                layout: new_layout,
            }),
            material_layout,
            writer: Default::default(),
        }
    }

    fn write_material(
        &mut self,
        pass: MaterialPass,
        resources: &MaterialResources,
        desc_alloc: &mut DescriptorAllocatorGrowable,
    ) -> MaterialInstance {
        let mat_data = MaterialInstance {
            pipeline: Rc::downgrade(if let MaterialPass::Transparent = pass {
                &self.pipeline_transparent
            } else {
                &self.pipeline_opaque
            }),
            material_set: desc_alloc.allocate(self.material_layout),
            pass_type: pass,
        };

        self.writer.clear();
        self.writer.write_buffer(
            0,
            resources.data_buffer,
            size_of::<MaterialConstants>() as u64,
            resources.data_buffer_offset as u64,
            vk::DescriptorType::UNIFORM_BUFFER,
        );
        self.writer.write_image(
            1,
            resources.color_img.img_view,
            resources.color_sampler,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        );
        self.writer.write_image(
            2,
            resources.metal_rough_img.img_view,
            resources.metal_rough_sampler,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        );

        self.writer
            .update_set(&self.device_copy, mat_data.material_set);

        mat_data
    }

    pub fn create_material(
        &mut self,
        allocator: Arc<Mutex<Allocator>>,
        textures: &Textures,
        global_desc_alloc: &mut DescriptorAllocatorGrowable,
    ) -> (AllocatedBuffer, MaterialInstance) {
        let material_constants = AllocatedBuffer::new(
            allocator,
            size_of::<MaterialConstants>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            MyMemoryUsage::CpuToGpu,
        );

        let scene_uniform_data = unsafe {
            mem::transmute::<*mut c_void, &mut MaterialConstants>(material_constants.mapped_data())
        };
        *scene_uniform_data = MaterialConstants {
            color_factors: Vec4::splat(1.),
            metal_rough_factors: glam::vec4(1., 0.5, 0., 0.),
        };

        let material_resources = MaterialResources {
            color_img: &textures.white,
            color_sampler: textures.default_sampler_linear,
            metal_rough_img: &textures.white,
            metal_rough_sampler: textures.default_sampler_linear,
            data_buffer: material_constants.buffer,
            data_buffer_offset: 0,
        };

        let instance = self.write_material(
            MaterialPass::MainColor,
            &material_resources,
            global_desc_alloc,
        );

        (material_constants, instance)
    }
}
