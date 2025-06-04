use std::{
    mem,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::{Device, util, vk};
use glam::{Mat4, Vec3, Vec4, vec3, vec4};
use vk_mem::Alloc;

use super::{
    commands::VulkanCommands,
    shaders_loader::{ShaderName, ShadersLoader},
};

pub struct VkGraphicsPipeline {
    device_copy: Rc<Device>,

    pub triangle_pipeline: vk::Pipeline,
    pub triangle_pipeline_layout: vk::PipelineLayout,

    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    pub mesh_buffers: GpuMeshBuffers,

    pub test_meshes: Vec<MeshAsset>,
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

        let create_info =
            vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&buffer_ranges[..]);
        let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None).unwrap() };
        let vertex_shader = shaders.get(ShaderName::ColoredTriangleMeshVert);
        let fragment_shader = shaders.get(ShaderName::ColoredTriangleFrag);

        let mut builder = PipelineBuilder {
            pipeline_layout,
            ..Default::default()
        };
        builder.set_shaders(vertex_shader.module_copy(), fragment_shader.module_copy());
        builder.set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        builder.set_polygon_mode(vk::PolygonMode::FILL);
        builder.set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE);
        builder.set_multisampling_none();
        builder.disable_blending();
        builder.enable_depthtest(true, vk::CompareOp::GREATER_OR_EQUAL);
        let draw_formats = [draw_format];
        builder.set_color_attachment_format(&draw_formats);
        builder.set_depth_format(depth_format);

        let pipeline = builder.clone().build(&device);

        let (vertices, indices) = default_buffer_data();
        let mesh_buffers = GpuMeshBuffers::new(&device, commands, &indices[..], &vertices[..]);

        let (triangle_pipeline, triangle_pipeline_layout) =
            Self::shader_with_hardcoded_mesh(shaders, &device, draw_format, depth_format);

        // TODO:proper resource path and all mngmt
        let test_meshes = load_gltf_meshes(&device, commands, "./resources/basicmesh.glb");

        Self {
            device_copy: device,
            triangle_pipeline,
            triangle_pipeline_layout,
            pipeline,
            pipeline_layout,
            mesh_buffers,
            test_meshes,
        }
    }

    /// Triangle is hardcoded in vertex shader
    fn shader_with_hardcoded_mesh(
        shaders: &ShadersLoader,
        device: &Device,
        draw_format: vk::Format,
        depth_format: vk::Format,
    ) -> (vk::Pipeline, vk::PipelineLayout) {
        let create_info = vk::PipelineLayoutCreateInfo::default();
        let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None).unwrap() };
        let vertex_shader = shaders.get(ShaderName::ColoredTriangleVert);
        let fragment_shader = shaders.get(ShaderName::ColoredTriangleFrag);

        let mut builder = PipelineBuilder {
            pipeline_layout,
            ..Default::default()
        };
        builder.set_shaders(vertex_shader.module_copy(), fragment_shader.module_copy());
        builder.set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        builder.set_polygon_mode(vk::PolygonMode::FILL);
        builder.set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE);
        builder.set_multisampling_none();
        builder.disable_blending();
        builder.enable_depthtest(true, vk::CompareOp::GREATER_OR_EQUAL);
        let formats = [draw_format];
        builder.set_color_attachment_format(&formats);
        builder.set_depth_format(depth_format);

        let pipeline = builder.clone().build(device);

        (pipeline, pipeline_layout)
    }
}

impl Drop for VkGraphicsPipeline {
    fn drop(&mut self) {
        println!("drop VkGraphicsPipeline");
        unsafe {
            self.device_copy.device_wait_idle().unwrap();

            self.device_copy
                .destroy_pipeline(self.triangle_pipeline, None);
            self.device_copy
                .destroy_pipeline_layout(self.triangle_pipeline_layout, None);

            self.device_copy.destroy_pipeline(self.pipeline, None);
            self.device_copy
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}

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
}

struct AllocatedBuffer {
    allocator_copy: Arc<Mutex<vk_mem::Allocator>>,
    pub buffer: vk::Buffer,
    allocation: vk_mem::Allocation,
    info: vk_mem::AllocationInfo,
}

pub enum MyMemoryUsage {
    GpuOnly,
    StagingUpload,
}

impl AllocatedBuffer {
    pub fn new(
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        alloc_size: u64,
        usage: vk::BufferUsageFlags,
        memory_usage: MyMemoryUsage,
    ) -> Self {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(alloc_size)
            .usage(usage);

        let mut alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            ..Default::default()
        };

        match memory_usage {
            MyMemoryUsage::GpuOnly => {
                alloc_info.required_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;
                // TODO: or usage : AutoPreferDevice ?
                // TODO: Consider using vk_mem::AllocationCreateFlags::DEDICATED_MEMORY,
                // especially if large
            }
            MyMemoryUsage::StagingUpload => {
                // When using MemoryUsage::Auto + MAPPED, needs one of :
                // #VMA_ALLOCATION_CREATE_HOST_ACCESS_SEQUENTIAL_WRITE_BIT
                // or #VMA_ALLOCATION_CREATE_HOST_ACCESS_RANDOM_BIT
                // requires memcpy and no random access (no mapped_data[i] = ...) !
                alloc_info.flags = vk_mem::AllocationCreateFlags::MAPPED
                    | vk_mem::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE;
            }
        }

        let (buffer, allocation, info) = {
            let allocator = allocator.lock().unwrap();
            unsafe {
                let (buffer, allocation) =
                    allocator.create_buffer(&buffer_info, &alloc_info).unwrap();
                let info = allocator.get_allocation_info(&allocation);
                println!("{info:?}");
                (buffer, allocation, info)
            }
        };

        Self {
            allocator_copy: allocator,
            buffer,
            allocation,
            info,
        }
    }
}

impl Drop for AllocatedBuffer {
    fn drop(&mut self) {
        println!("drop AllocatedBuffer");
        unsafe {
            self.allocator_copy
                .lock()
                .unwrap()
                .destroy_buffer(self.buffer, &mut self.allocation);
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Vertex {
    position: Vec3,
    uv_x: f32,
    normal: Vec3,
    uv_y: f32,
    color: Vec4,
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: Default::default(),
            uv_x: Default::default(),
            normal: vec3(1., 0., 0.),
            uv_y: Default::default(),
            color: vec4(1., 1., 1., 1.),
        }
    }
}

impl Vertex {
    pub fn from_position(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: Vec3 { x, y, z },
            ..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct GpuDrawPushConstants {
    pub world_mat: Mat4,
    pub vertex_buffer: vk::DeviceAddress,
}

pub struct GpuMeshBuffers {
    index_buffer: AllocatedBuffer,
    _vertex_buffer: AllocatedBuffer,
    pub vertex_buffer_address: vk::DeviceAddress,
}

impl GpuMeshBuffers {
    fn new(
        device: &Device,
        commands: &VulkanCommands,
        indices: &[u32],
        vertices: &[Vertex],
    ) -> Self {
        let vertex_buffer_size = size_of_val(vertices) as u64;
        let index_buffer_size = size_of_val(indices) as u64;

        let vertex_buffer = AllocatedBuffer::new(
            commands.allocator.clone(),
            vertex_buffer_size,
            vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_DST
                | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            MyMemoryUsage::GpuOnly,
        );

        let device_address_info =
            vk::BufferDeviceAddressInfo::default().buffer(vertex_buffer.buffer);
        let vertex_buffer_address =
            unsafe { device.get_buffer_device_address(&device_address_info) };

        let index_buffer = AllocatedBuffer::new(
            commands.allocator.clone(),
            index_buffer_size,
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            MyMemoryUsage::GpuOnly,
        );

        // TODO: check https://gpuopen-librariesandsdks.github.io/VulkanMemoryAllocator/html/usage_patterns.html
        // esp Advanced data uploading for APU without staging and stuff...

        let staging = AllocatedBuffer::new(
            commands.allocator.clone(),
            vertex_buffer_size + index_buffer_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MyMemoryUsage::StagingUpload,
        );

        let data = staging.info.mapped_data;
        let mut align =
            unsafe { util::Align::new(data, mem::align_of::<Vertex>() as _, vertex_buffer_size) };
        align.copy_from_slice(vertices);
        // TODO: can alignment break sizes ?
        let mut align = unsafe {
            util::Align::new(
                data.add(vertex_buffer_size as usize),
                mem::align_of::<u32>() as _,
                index_buffer_size,
            )
        };
        align.copy_from_slice(indices);

        // TODO: can be sent to background thread to avoid blocking
        commands.immediate_submit(|device, cmd| {
            let vertex_copies = [vk::BufferCopy::default()
                .dst_offset(0)
                .src_offset(0)
                .size(vertex_buffer_size)];
            unsafe {
                device.cmd_copy_buffer(
                    cmd,
                    staging.buffer,
                    vertex_buffer.buffer,
                    &vertex_copies[..],
                );
            }

            let index_copies = [vk::BufferCopy::default()
                .dst_offset(0)
                .src_offset(vertex_buffer_size)
                .size(index_buffer_size)];
            unsafe {
                device.cmd_copy_buffer(cmd, staging.buffer, index_buffer.buffer, &index_copies[..]);
            }
        });

        Self {
            index_buffer,
            _vertex_buffer: vertex_buffer,
            vertex_buffer_address,
        }
    }

    pub fn index_buffer(&self) -> &vk::Buffer {
        &self.index_buffer.buffer
    }
}

fn default_buffer_data() -> ([Vertex; 4], [u32; 6]) {
    let rect_vertices = [
        Vertex::from_position(0.5, -0.5, 0.),
        Vertex::from_position(0.5, 0.5, 0.),
        Vertex::from_position(-0.5, -0.5, 0.),
        Vertex::from_position(-0.5, 0.5, 0.),
    ];
    let rect_indices = [0, 1, 2, 2, 1, 3];
    (rect_vertices, rect_indices)
}

// glTF
// TODO: move ?

#[derive(Default, Debug, Clone, Copy)]
pub struct GeoSurface {
    pub start_index: u32,
    pub count: u32,
}

pub struct MeshAsset {
    pub _name: Option<String>,
    pub surfaces: Vec<GeoSurface>,
    pub mesh_buffers: GpuMeshBuffers,
}

fn load_gltf_meshes(
    device: &Device,
    commands: &VulkanCommands,
    path: impl AsRef<Path>,
) -> Vec<MeshAsset> {
    let (document, buffers, _) = gltf::import(path).unwrap();

    // In common to prevent reallocating much
    let mut indices = Vec::new();
    let mut vertices = Vec::new();

    document
        .meshes()
        .map(|mesh| {
            indices.clear();
            vertices.clear();

            let name = mesh.name().map(|n| n.to_string());
            let surfaces = mesh
                .primitives()
                .filter_map(|p| p.indices().map(|i| (p, i)))
                .map(|(primitive, index_accessor)| {
                    let count = index_accessor.count();

                    let surface = GeoSurface {
                        start_index: indices.len() as u32,
                        count: count as u32,
                    };

                    let initial_vtx = vertices.len();
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                    indices.reserve(count);
                    // TODO: what to do if None ?
                    if let Some(iter) = reader.read_indices() {
                        indices.extend(iter.into_u32().map(|i| i + initial_vtx as u32));
                    }

                    // TODO: what to do if None ?
                    if let Some(pos_accessor) = primitive.get(&gltf::Semantic::Positions) {
                        vertices.reserve(pos_accessor.count());
                        // TODO: what to do if None ?
                        if let Some(iter) = reader.read_positions() {
                            vertices.extend(iter.map(|p| Vertex {
                                position: Vec3::from_array(p),
                                ..Default::default()
                            }));
                        }
                    }

                    if let Some(iter) = reader.read_normals() {
                        iter.enumerate().for_each(|(i, n)| {
                            vertices[initial_vtx + i].normal = Vec3::from_array(n)
                        });
                    }

                    // TODO: 0?
                    if let Some(iter) = reader.read_tex_coords(0) {
                        iter.into_f32().enumerate().for_each(|(i, c)| {
                            let v = &mut vertices[initial_vtx + i];
                            v.uv_x = c[0];
                            v.uv_y = c[1];
                        });
                    }

                    // TODO: 0?
                    if let Some(iter) = reader.read_colors(0) {
                        iter.into_rgba_f32().enumerate().for_each(|(i, c)| {
                            vertices[initial_vtx + i].color = Vec4::from_array(c)
                        });
                    }

                    surface
                })
                .collect();

            let override_colors = true;
            if override_colors {
                vertices
                    .iter_mut()
                    .for_each(|v| v.color = v.normal.extend(1.));
            }

            let mesh_buffers = GpuMeshBuffers::new(device, commands, &indices[..], &vertices[..]);

            MeshAsset {
                _name: name,
                surfaces,
                mesh_buffers,
            }
        })
        .collect()
}
