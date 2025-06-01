use std::rc::Rc;

use ash::{Device, vk};

use super::vulkan_shaders::{ShaderName, VulkanShaders};

#[derive(Default, Debug, Clone)]
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
    pub fn build(mut self, device: &Device) -> vk::Pipeline {
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

    pub fn set_shaders(
        &mut self,
        vertex_shader: vk::ShaderModule,
        fragment_shader: vk::ShaderModule,
    ) {
        let vertex_shader = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_shader)
            .name(c"main");
        let fragment_shader = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_shader)
            .name(c"main");

        self.shader_stages.push(vertex_shader);
        self.shader_stages.push(fragment_shader);
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

    pub fn disable_blending(&mut self) {
        self.color_blend_attachment = self
            .color_blend_attachment
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false);
    }

    pub fn set_color_attachment_format(&mut self, formats: &'a [vk::Format; 1]) {
        // self.color_attachment_formats = *formats;
        self.render_info = self.render_info.color_attachment_formats(&formats[..]);
    }

    pub fn set_depth_format(&mut self, format: vk::Format) {
        self.render_info = self.render_info.depth_attachment_format(format);
    }

    pub fn disable_depthtest(&mut self) {
        self.depth_stencil = self
            .depth_stencil
            .depth_test_enable(false)
            .depth_write_enable(false)
            .depth_compare_op(vk::CompareOp::NEVER)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false)
            .min_depth_bounds(0.)
            .max_depth_bounds(1.);
    }
}

pub struct VkGraphicsPipeline {
    device_copy: Rc<Device>,
    pub pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
}

impl VkGraphicsPipeline {
    pub fn new(shaders: &VulkanShaders, device: Rc<Device>, draw_format: vk::Format) -> Self {
        let create_info = vk::PipelineLayoutCreateInfo::default();
        let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None).unwrap() };
        let vertex_shader = shaders.get(ShaderName::ColoredTriangleVert);
        let fragment_shader = shaders.get(ShaderName::ColoredTriangleFrag);

        let mut builder = PipelineBuilder {
            pipeline_layout,
            ..Default::default()
        };
        builder.set_shaders(vertex_shader, fragment_shader);
        builder.set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        builder.set_polygon_mode(vk::PolygonMode::FILL);
        builder.set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE);
        builder.set_multisampling_none();
        builder.disable_blending();
        builder.disable_depthtest();
        let formats = [draw_format];
        builder.set_color_attachment_format(&formats);
        builder.set_depth_format(vk::Format::UNDEFINED);

        let pipeline = builder.clone().build(&device);

        Self {
            device_copy: device,
            pipeline,
            pipeline_layout,
        }
    }
}

impl Drop for VkGraphicsPipeline {
    fn drop(&mut self) {
        println!("drop VkGraphicsPipeline");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();
            self.device_copy.destroy_pipeline(self.pipeline, None);
            self.device_copy
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
