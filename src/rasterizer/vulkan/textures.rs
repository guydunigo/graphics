use std::{
    ops::Deref,
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

use ash::{Device, vk};
use glam::Vec4;
use vk_mem::Allocator;

use super::{
    allocated::{AllocatedBuffer, AllocatedImage, MyMemoryUsage},
    commands::VulkanCommands,
    descriptors::{DescriptorAllocatorGrowable, DescriptorLayoutBuilder, DescriptorWriter},
    gfx_pipeline::{GpuDrawPushConstants, PipelineBuilder},
    shaders_loader::{ShaderName, ShadersLoader},
    swapchain::VulkanSwapchain,
};

pub struct Textures<'a> {
    device_copy: Rc<Device>,

    pub white: Rc<AllocatedImage>,
    pub grey: Rc<AllocatedImage>,
    pub black: Rc<AllocatedImage>,
    pub error_checkerboard: Rc<AllocatedImage>,

    pub default_sampler_linear: vk::Sampler,
    pub default_sampler_nearest: vk::Sampler,

    pub default_material: Rc<MaterialInstance>,
    _material_constants: AllocatedBuffer,
    pub metal_rough_material: GltfMetallicRoughness<'a>,
    _global_desc_alloc: DescriptorAllocatorGrowable,
}

impl Textures<'_> {
    pub fn new(
        swapchain: &VulkanSwapchain,
        commands: &VulkanCommands,
        shaders: &ShadersLoader,
        device: Rc<Device>,
        allocator: Arc<Mutex<Allocator>>,
        scene_data_descriptor_layout: vk::DescriptorSetLayout,
    ) -> Self {
        let extent = vk::Extent3D {
            width: 1,
            height: 1,
            depth: 1,
        };
        let format = vk::Format::R8G8B8A8_UNORM;
        let usages = vk::ImageUsageFlags::SAMPLED;

        let white = {
            let data = glam::U8Vec4::splat(255).to_array();
            AllocatedImage::new_and_upload(
                commands,
                device.clone(),
                allocator.clone(),
                extent,
                format,
                usages,
                false,
                &data[..],
            )
        };

        let black_data = glam::U8Vec4::splat(0).to_array();
        let black = {
            AllocatedImage::new_and_upload(
                commands,
                device.clone(),
                allocator.clone(),
                extent,
                format,
                usages,
                false,
                &black_data[..],
            )
        };

        let grey = {
            let data = glam::U8Vec4::splat((255. * 0.66) as u8).to_array();
            AllocatedImage::new_and_upload(
                commands,
                device.clone(),
                allocator.clone(),
                extent,
                format,
                usages,
                false,
                &data[..],
            )
        };

        let error_checkerboard = {
            let extent = vk::Extent3D {
                width: 16,
                height: 16,
                depth: 1,
            };
            let magenta_data = glam::u8vec4(255, 0, 255, 255).to_array();
            let mut pixels: [u8; 16 * 16 * 4] = [0; 1024];
            for y in 0..16 {
                for x in 0..16 {
                    let index = y * 16 * 4 + x * 4;
                    if (x % 2) ^ (y % 2) == 0 {
                        pixels[index..index + 4].copy_from_slice(&black_data[..]);
                    } else {
                        pixels[index..index + 4].copy_from_slice(&magenta_data[..]);
                    };
                }
            }

            AllocatedImage::new_and_upload(
                commands,
                device.clone(),
                allocator.clone(),
                extent,
                format,
                usages,
                false,
                &pixels[..],
            )
        };

        let default_sampler_linear = {
            let create_info = vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR);
            unsafe { device.create_sampler(&create_info, None).unwrap() }
        };
        let default_sampler_nearest = {
            let create_info = vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST);
            unsafe { device.create_sampler(&create_info, None).unwrap() }
        };

        let mut metal_rough_material = GltfMetallicRoughness::new(
            device.clone(),
            shaders,
            *swapchain.draw_format(),
            *swapchain.depth_format(),
            scene_data_descriptor_layout,
        );

        // See `resources/input_structures.glsl` :
        // - UNIFORM_BUFFER for SceneData
        // - ''             for GLTFMaterialData
        // - img sampler    for colorTex
        // - ''             for metalRoughTex
        let sizes = vec![
            (vk::DescriptorType::UNIFORM_BUFFER, 2.),
            (vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 2.),
        ];
        let mut global_desc_alloc = DescriptorAllocatorGrowable::new(device.clone(), 10, sizes);

        let (material_constants, default_material) = metal_rough_material.create_material(
            allocator.clone(),
            &mut global_desc_alloc,
            &error_checkerboard,
            default_sampler_nearest,
        );

        Self {
            device_copy: device,

            white: Rc::new(white),
            grey: Rc::new(grey),
            black: Rc::new(black),
            error_checkerboard: Rc::new(error_checkerboard),
            default_sampler_linear,
            default_sampler_nearest,

            default_material: Rc::new(default_material),
            _material_constants: material_constants,
            metal_rough_material,
            _global_desc_alloc: global_desc_alloc,
        }
    }
}

impl Drop for Textures<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "vulkan_dbg_mem")]
        println!("drop Textures");
        unsafe {
            self.device_copy
                .destroy_sampler(self.default_sampler_linear, None);
            self.device_copy
                .destroy_sampler(self.default_sampler_nearest, None);
        }
    }
}

/// This struct should be the master of the pipelines and layouts,
/// it takes care of destroying the layout on drop.
pub struct GltfMetallicRoughness<'a> {
    device_copy: Rc<Device>,

    pipeline_opaque: Rc<MaterialPipeline>,
    pipeline_transparent: Rc<MaterialPipeline>,

    material_layout: vk::DescriptorSetLayout,

    writer: DescriptorWriter<'a>,
}

impl Drop for GltfMetallicRoughness<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "vulkan_dbg_mem")]
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
    pub fn new(
        device: Rc<Device>,
        shaders: &ShadersLoader,
        draw_img_format: vk::Format,
        depth_img_format: vk::Format,
        scene_data_descriptor_layout: vk::DescriptorSetLayout,
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
        let layouts = [scene_data_descriptor_layout, material_layout];

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

    pub fn write_material(
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
        global_desc_alloc: &mut DescriptorAllocatorGrowable,
        texture: &AllocatedImage,
        sampler: vk::Sampler,
    ) -> (AllocatedBuffer, MaterialInstance) {
        let material_constants = AllocatedBuffer::new(
            allocator,
            size_of::<MaterialConstants>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            MyMemoryUsage::CpuToGpu,
        );

        let scene_uniform_data =
            unsafe { &mut *material_constants.mapped_data().cast::<MaterialConstants>() };
        *scene_uniform_data = MaterialConstants {
            color_factors: Vec4::splat(1.),
            metal_rough_factors: glam::vec4(1., 0.5, 0., 0.),
        };

        let material_resources = MaterialResources {
            color_img: texture,
            color_sampler: sampler,
            metal_rough_img: texture,
            metal_rough_sampler: sampler,
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

/// The fields are supposed to be destroyed by the parent class GltfMetallicRoughness
pub struct MaterialPipeline {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
}

pub enum MaterialPass {
    MainColor,
    Transparent,
    Other,
}

pub struct MaterialInstance {
    pipeline: Weak<MaterialPipeline>,
    pub material_set: vk::DescriptorSet,
    pass_type: MaterialPass,
}

impl MaterialInstance {
    pub fn pipeline(&self) -> impl Deref<Target = MaterialPipeline> {
        self.pipeline.upgrade().unwrap()
    }

    pub fn pass_type(&self) -> &MaterialPass {
        &self.pass_type
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
// TODO: align 256, and in copy slices, ... ?
// #[repr(align(256))]
pub struct MaterialConstants {
    pub color_factors: Vec4,
    pub metal_rough_factors: Vec4,
}

pub struct MaterialResources<'a> {
    pub color_img: &'a AllocatedImage,
    pub color_sampler: vk::Sampler,
    pub metal_rough_img: &'a AllocatedImage,
    pub metal_rough_sampler: vk::Sampler,
    pub data_buffer: vk::Buffer,
    pub data_buffer_offset: u32,
}
