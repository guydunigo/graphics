use std::rc::Rc;

use ash::{Device, vk};
use glam::Mat4;

use super::{
    commands::VulkanCommands,
    descriptors::DescriptorLayoutBuilder,
    gltf_loader::{MeshAsset, load_gltf_meshes},
    shaders_loader::{ShaderModule, ShaderName, ShadersLoader},
};

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct GpuDrawPushConstants {
    pub world_mat: Mat4,
    pub vertex_buffer: vk::DeviceAddress,
}

pub struct VkGraphicsPipeline {
    device_copy: Rc<Device>,

    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,

    pub meshes: Vec<MeshAsset>,

    pub single_image_descriptor_layout: vk::DescriptorSetLayout,
}

impl VkGraphicsPipeline {
    pub fn new(
        commands: &VulkanCommands,
        shaders: &ShadersLoader,
        device: Rc<Device>,
        draw_format: vk::Format,
        depth_format: vk::Format,
    ) -> Self {
        let buffer_ranges = [vk::PushConstantRange::default()
            .offset(0)
            .size(size_of::<GpuDrawPushConstants>() as u32)
            .stage_flags(vk::ShaderStageFlags::VERTEX)];

        let single_image_descriptor_layout = DescriptorLayoutBuilder::default()
            .add_binding(0, vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .build(&device, vk::ShaderStageFlags::FRAGMENT);
        let sid_layouts = [single_image_descriptor_layout];

        let create_info = vk::PipelineLayoutCreateInfo::default()
            .push_constant_ranges(&buffer_ranges[..])
            .set_layouts(&sid_layouts[..]);
        let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None).unwrap() };
        let vertex_shader = shaders.get(ShaderName::ColoredTriangleMeshVert);
        let fragment_shader = shaders.get(ShaderName::TexImage);

        let mut builder = PipelineBuilder::new(pipeline_layout);
        builder.set_shaders(&vertex_shader, &fragment_shader);
        builder.set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        builder.set_polygon_mode(vk::PolygonMode::FILL);
        builder.set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE);
        builder.set_multisampling_none();
        builder.enable_blending_additive();
        builder.enable_depthtest(true, vk::CompareOp::GREATER_OR_EQUAL);
        let formats = [draw_format];
        builder.set_color_attachment_format(&formats[..]);
        builder.set_depth_format(depth_format);

        let pipeline = builder.build(&device);

        // TODO: proper resource path and all mngmt
        let meshes = load_gltf_meshes(&device, commands, "./resources/basicmesh.glb");

        Self {
            device_copy: device,
            pipeline,
            pipeline_layout,
            meshes,
            single_image_descriptor_layout,
        }
    }

    // /// Triangle is hardcoded in vertex shader
    // fn shader_with_hardcoded_mesh(
    //     shaders: &ShadersLoader,
    //     device: &Device,
    //     draw_format: vk::Format,
    //     depth_format: vk::Format,
    // ) -> (vk::Pipeline, vk::PipelineLayout) {
    //     let create_info = vk::PipelineLayoutCreateInfo::default();
    //     let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None).unwrap() };
    //     let vertex_shader = shaders.get(ShaderName::ColoredTriangleVert);
    //     let fragment_shader = shaders.get(ShaderName::ColoredTriangleFrag);

    //     let mut builder = PipelineBuilder {
    //         pipeline_layout,
    //         ..Default::default()
    //     };
    //     builder.set_shaders(vertex_shader.module_copy(), fragment_shader.module_copy());
    //     builder.set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    //     builder.set_polygon_mode(vk::PolygonMode::FILL);
    //     builder.set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE);
    //     builder.set_multisampling_none();
    //     builder.disable_blending();
    //     builder.enable_depthtest(true, vk::CompareOp::GREATER_OR_EQUAL);
    //     let formats = [draw_format];
    //     builder.set_color_attachment_format(&formats);
    //     builder.set_depth_format(depth_format);

    //     let pipeline = builder.clone().build(device);

    //     (pipeline, pipeline_layout)
    // }
}

impl Drop for VkGraphicsPipeline {
    fn drop(&mut self) {
        println!("drop VkGraphicsPipeline");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();

            self.device_copy.destroy_pipeline(self.pipeline, None);
            self.device_copy
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device_copy
                .destroy_descriptor_set_layout(self.single_image_descriptor_layout, None);
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineBuilder<'a> {
    shader_stages: Vec<vk::PipelineShaderStageCreateInfo<'a>>,

    input_assembly: vk::PipelineInputAssemblyStateCreateInfo<'a>,
    rasterizer: vk::PipelineRasterizationStateCreateInfo<'a>,
    color_blend_attachment: vk::PipelineColorBlendAttachmentState,
    multisampling: vk::PipelineMultisampleStateCreateInfo<'a>,
    pipeline_layout: vk::PipelineLayout,
    depth_stencil: vk::PipelineDepthStencilStateCreateInfo<'a>,
    render_info: vk::PipelineRenderingCreateInfo<'a>,
    // color_attachment_formats: [vk::Format; 1],
}

impl<'a> PipelineBuilder<'a> {
    // TODO: not ideal since we default all components...
    pub fn new(pipeline_layout: vk::PipelineLayout) -> Self {
        Self {
            shader_stages: Default::default(),
            input_assembly: Default::default(),
            rasterizer: Default::default(),
            color_blend_attachment: Default::default(),
            multisampling: Default::default(),
            pipeline_layout,
            depth_stencil: Default::default(),
            render_info: Default::default(),
        }
    }
    pub fn build(&mut self, device: &Device) -> vk::Pipeline {
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        // For now, no transparancy, disabled :
        let color_blend_attachments = [self.color_blend_attachment];
        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(&color_blend_attachments[..]);

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default();

        let state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_info = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&state[..]);

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .push_next(&mut self.render_info)
            .stages(&self.shader_stages[..])
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&self.input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&self.rasterizer)
            .multisample_state(&self.multisampling)
            .color_blend_state(&color_blending)
            .depth_stencil_state(&self.depth_stencil)
            .layout(self.pipeline_layout)
            .dynamic_state(&dynamic_info);
        let pipeline_infos = [pipeline_info];

        unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_infos[..], None)
                .unwrap()[0]
        }
    }

    pub fn set_shaders(&mut self, vertex_shader: &ShaderModule, fragment_shader: &ShaderModule) {
        let vertex_shader_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_shader.module_copy())
            .name(c"main");
        let fragment_shader_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_shader.module_copy())
            .name(c"main");

        self.shader_stages.push(vertex_shader_stage);
        self.shader_stages.push(fragment_shader_stage);
    }

    pub fn set_input_topology(&mut self, topology: vk::PrimitiveTopology) {
        self.input_assembly = self
            .input_assembly
            .topology(topology)
            .primitive_restart_enable(false);
    }

    pub fn set_polygon_mode(&mut self, mode: vk::PolygonMode) {
        self.rasterizer = self.rasterizer.polygon_mode(mode).line_width(1.);
    }

    pub fn set_cull_mode(&mut self, cull_mode: vk::CullModeFlags, front_face: vk::FrontFace) {
        self.rasterizer = self.rasterizer.cull_mode(cull_mode).front_face(front_face);
    }

    pub fn set_multisampling_none(&mut self) {
        self.multisampling = self
            .multisampling
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            // 1 sample per pixel
            .min_sample_shading(1.)
            .sample_mask(&[])
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false);
    }

    pub fn set_color_attachment_format(&mut self, formats: &'a [vk::Format]) {
        self.render_info = self.render_info.color_attachment_formats(formats);
    }

    pub fn set_depth_format(&mut self, format: vk::Format) {
        self.render_info = self.render_info.depth_attachment_format(format);
    }

    // pub fn disable_depthtest(&mut self) {
    //     self.depth_stencil = self
    //         .depth_stencil
    //         .depth_test_enable(false)
    //         .depth_write_enable(false)
    //         .depth_compare_op(vk::CompareOp::NEVER)
    //         .depth_bounds_test_enable(false)
    //         .stencil_test_enable(false)
    //         .min_depth_bounds(0.)
    //         .max_depth_bounds(1.);
    // }

    pub fn enable_depthtest(&mut self, depth_write_enable: bool, op: vk::CompareOp) {
        self.depth_stencil = self
            .depth_stencil
            .depth_test_enable(true)
            .depth_write_enable(depth_write_enable)
            .depth_compare_op(op)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .min_depth_bounds(0.)
            .max_depth_bounds(1.);
    }

    pub fn disable_blending(&mut self) {
        self.color_blend_attachment = self
            .color_blend_attachment
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false);
    }

    fn enable_blending_base(&mut self, dst_color_blend_factor: vk::BlendFactor) {
        self.color_blend_attachment = self
            .color_blend_attachment
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(dst_color_blend_factor)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);
    }

    pub fn enable_blending_additive(&mut self) {
        self.enable_blending_base(vk::BlendFactor::ONE);
    }

    // pub fn enable_blending_alphablend(&mut self) {
    //     self.enable_blending_base(vk::BlendFactor::ONE_MINUS_SRC_ALPHA);
    // }
}
