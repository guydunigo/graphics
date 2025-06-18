use std::{
    cell::RefCell,
    collections::HashMap,
    mem,
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

use super::{
    allocated::{AllocatedBuffer, MyMemoryUsage},
    commands::VulkanCommands,
    descriptors::{DescriptorLayoutBuilder, DescriptorWriter},
    gfx_pipeline::GpuDrawPushConstants,
    gltf_loader::load_gltf_meshes,
    shaders_loader::ShadersLoader,
    swapchain::VulkanSwapchain,
    textures::{MaterialInstance, Textures},
};

use ash::{Device, util::Align, vk};
use glam::{Mat4, Vec3, Vec4, vec3, vec4};

pub struct Scene<'a> {
    device_copy: Rc<Device>,

    nodes: HashMap<String, Rc<RefCell<dyn Node>>>,
    _textures: Textures<'a>,

    data: GpuSceneData,
    pub data_descriptor_layout: vk::DescriptorSetLayout,
    // gpu_scene_data_buffer: Option<AllocatedBuffer>,
    pub main_draw_ctx: DrawContext,
}

impl Scene<'_> {
    pub fn new(
        swapchain: &VulkanSwapchain,
        commands: &VulkanCommands,
        shaders: &ShadersLoader,
        device: Rc<Device>,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
    ) -> Self {
        let data_descriptor_layout = DescriptorLayoutBuilder::default()
            .add_binding(0, vk::DescriptorType::UNIFORM_BUFFER)
            .build(
                &device,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            );

        let textures = Textures::new(
            swapchain,
            commands,
            shaders,
            device.clone(),
            allocator,
            data_descriptor_layout,
        );

        // TODO: proper resource path mngmt and all
        let mut meshes = load_gltf_meshes(
            &device,
            commands,
            textures.default_material.clone(),
            "./resources/basicmesh.glb",
        );

        let nodes = meshes
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
            device_copy: device.clone(),

            nodes,
            _textures: textures,

            data: Default::default(),
            data_descriptor_layout,
            // gpu_scene_data_buffer: Default::default(),
            main_draw_ctx: Default::default(),
        }
    }

    pub fn upload_data(
        &self,
        device: &Device,
        allocator: Arc<Mutex<vk_mem::Allocator>>,
        global_desc: vk::DescriptorSet,
    ) {
        // We will also dynamically allocate the uniform buffer itself as a way to
        // showcase how you could do temporal per-frame data that is dynamically created.
        // It would be better to hold the buffers cached in our FrameData structure,
        // but we will be doing it this way to show how.
        // There are cases with dynamic draws and passes where you might want to do it
        // this way.
        let gpu_scene_data_buffer = AllocatedBuffer::new(
            allocator,
            size_of::<GpuSceneData>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            MyMemoryUsage::CpuToGpu,
        );
        let scene_data =
            unsafe { &mut *gpu_scene_data_buffer.mapped_data().cast::<GpuSceneData>() };
        *scene_data = self.data;

        let mut writer = DescriptorWriter::default();
        writer.write_buffer(
            0,
            gpu_scene_data_buffer.buffer,
            size_of::<GpuSceneData>() as u64,
            0,
            vk::DescriptorType::UNIFORM_BUFFER,
        );
        writer.update_set(device, global_desc);

        // TODO: store until next frame so GPU has time to use it ?
        // self.gpu_scene_data_buffer = Some(gpu_scene_data_buffer);
    }

    /// Clears the `main_draw_ctx` and fills it with the meshes to render.
    pub fn update_scene(&mut self, draw_extent: vk::Extent2D) {
        self.main_draw_ctx.clear();

        self.nodes["Suzanne"]
            .borrow()
            .draw(&Mat4::IDENTITY, &mut self.main_draw_ctx);

        {
            let cube = self.nodes["Cube"].borrow();
            (-3..3).for_each(|x| {
                let scale = Mat4::from_scale(Vec3::splat(0.2));
                let translation = Mat4::from_translation(vec3(x as f32, 1., 0.));

                cube.draw(&(translation * scale), &mut self.main_draw_ctx);
            });
        }

        let view = Mat4::from_translation(vec3(0., 0., -5.));
        // Camera projection
        let mut proj = Mat4::perspective_rh(
            70.,
            draw_extent.width as f32 / draw_extent.height as f32,
            10_000.,
            0.1,
        );
        proj.y_axis[1] *= -1.;
        self.data = GpuSceneData {
            view,
            proj,
            view_proj: proj * view,
            ambient_color: Vec4::splat(1.),
            sunlight_direction: vec4(0., 1., 0.5, 1.),
            sunlight_color: Vec4::splat(1.),
        };
    }
}

impl Drop for Scene<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "dbg_mem")]
        println!("drop Scene");
        unsafe {
            self.device_copy
                .destroy_descriptor_set_layout(self.data_descriptor_layout, None);
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct GpuSceneData {
    pub view: Mat4,
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub ambient_color: Vec4,
    pub sunlight_direction: Vec4,
    pub sunlight_color: Vec4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    pub uv_x: f32,
    pub normal: Vec3,
    pub uv_y: f32,
    pub color: Vec4,
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

// impl Vertex {
//     pub fn from_position(x: f32, y: f32, z: f32) -> Self {
//         Self {
//             position: Vec3 { x, y, z },
//             ..Default::default()
//         }
//     }
// }

pub struct GpuMeshBuffers {
    pub index_buffer: AllocatedBuffer,
    _vertex_buffer: AllocatedBuffer,
    pub vertex_buffer_address: vk::DeviceAddress,
}

impl GpuMeshBuffers {
    pub fn new(
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

        let data = staging.mapped_data();
        let mut align =
            unsafe { Align::new(data, mem::align_of::<Vertex>() as _, vertex_buffer_size) };
        align.copy_from_slice(vertices);
        // TODO: can alignment break sizes ?
        let mut align = unsafe {
            Align::new(
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

    // pub fn example(device: &Device, commands: &VulkanCommands) -> Self {
    //     let vertices = [
    //         Vertex::from_position(0.5, -0.5, 0.),
    //         Vertex::from_position(0.5, 0.5, 0.),
    //         Vertex::from_position(-0.5, -0.5, 0.),
    //         Vertex::from_position(-0.5, 0.5, 0.),
    //     ];
    //     let indices = [0, 1, 2, 2, 1, 3];
    //     Self::new(device, commands, &indices[..], &vertices[..])
    // }
}

pub struct GeoSurface {
    pub start_index: u32,
    pub count: u32,
    pub material: Rc<MaterialInstance>,
}

pub struct MeshAsset {
    pub name: String,
    pub surfaces: Vec<GeoSurface>,
    mesh_buffers: GpuMeshBuffers,
}

impl MeshAsset {
    pub fn new(name: String, surfaces: Vec<GeoSurface>, mesh_buffers: GpuMeshBuffers) -> Self {
        Self {
            name,
            surfaces,
            mesh_buffers,
        }
    }

    pub fn index_buffer(&self) -> &vk::Buffer {
        &self.mesh_buffers.index_buffer.buffer
    }

    pub fn vertex_buffer_address(&self) -> vk::DeviceAddress {
        self.mesh_buffers.vertex_buffer_address
    }
}

pub struct RenderObject {
    pub index_count: u32,
    pub first_index: u32,
    pub index_buffer: vk::Buffer,

    pub material: Rc<MaterialInstance>,

    transform: Mat4,
    vertex_buffer_addr: vk::DeviceAddress,
}

impl From<&RenderObject> for GpuDrawPushConstants {
    fn from(value: &RenderObject) -> Self {
        GpuDrawPushConstants {
            world_mat: value.transform,
            vertex_buffer: value.vertex_buffer_addr,
        }
    }
}

#[derive(Default)]
pub struct DrawContext {
    pub opaque_surfaces: Vec<RenderObject>,
}

impl DrawContext {
    pub fn clear(&mut self) {
        self.opaque_surfaces.clear();
    }
}

pub trait Renderable {
    fn draw(&self, top_mat: &Mat4, ctx: &mut DrawContext);
}

pub trait Node: Renderable {
    fn refresh_transform(&mut self, parent_mat: &Mat4);
}

struct EmptyNode;
impl Renderable for EmptyNode {
    fn draw(&self, _top_mat: &Mat4, _ctx: &mut DrawContext) {
        unreachable!()
    }
}
impl Node for EmptyNode {
    fn refresh_transform(&mut self, _parent_mat: &Mat4) {
        unreachable!()
    }
}

// TODO: or have Node contain a dyn Renderable/Node like MeshNode
pub struct NodeData {
    /// If there is no parent or it was destroyed, weak won't upgrade.
    parent: Weak<RefCell<dyn Node>>,
    children: Vec<Rc<RefCell<dyn Node>>>,
    local_transform: Mat4,
    world_transform: Mat4,
}

impl Renderable for NodeData {
    fn draw(&self, top_mat: &Mat4, ctx: &mut DrawContext) {
        self.children
            .iter()
            .for_each(|c| c.borrow().draw(top_mat, ctx));
    }
}

impl Node for NodeData {
    fn refresh_transform(&mut self, parent_mat: &Mat4) {
        self.world_transform = parent_mat * self.local_transform;
        self.children
            .iter()
            .for_each(|c| c.borrow_mut().refresh_transform(parent_mat));
    }
}

pub struct MeshNode {
    node: NodeData,

    mesh: Rc<MeshAsset>,
}

impl MeshNode {
    pub fn new(mesh: MeshAsset, local_transform: Mat4, world_transform: Mat4) -> Self {
        let parent: Weak<RefCell<EmptyNode>> = Weak::new();
        MeshNode {
            node: NodeData {
                parent,
                children: Default::default(),
                local_transform,
                world_transform,
            },
            mesh: Rc::new(mesh),
        }
    }
}

impl Renderable for MeshNode {
    fn draw(&self, top_mat: &Mat4, ctx: &mut DrawContext) {
        let node_mat = top_mat * self.node.world_transform;

        self.mesh.surfaces.iter().for_each(|s| {
            let def = RenderObject {
                index_count: s.count,
                first_index: s.start_index,
                index_buffer: *self.mesh.index_buffer(),
                material: s.material.clone(),
                transform: node_mat,
                vertex_buffer_addr: self.mesh.vertex_buffer_address(),
            };

            ctx.opaque_surfaces.push(def);
        });

        self.node.draw(top_mat, ctx);
    }
}

impl Node for MeshNode {
    fn refresh_transform(&mut self, parent_mat: &Mat4) {
        self.node.refresh_transform(parent_mat);
    }
}
