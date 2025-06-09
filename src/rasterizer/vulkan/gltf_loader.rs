use std::{mem, path::Path};

use ash::{Device, util::Align, vk};
use glam::{Vec3, Vec4, vec3, vec4};

use super::{
    commands::VulkanCommands,
    descriptors::{AllocatedBuffer, MyMemoryUsage},
};

pub struct MeshAsset {
    pub _name: Option<String>,
    pub surfaces: Vec<GeoSurface>,
    mesh_buffers: GpuMeshBuffers,
}

impl MeshAsset {
    pub fn index_buffer(&self) -> &vk::Buffer {
        &self.mesh_buffers.index_buffer.buffer
    }

    pub fn vertex_buffer_address(&self) -> vk::DeviceAddress {
        self.mesh_buffers.vertex_buffer_address
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

// impl Vertex {
//     pub fn from_position(x: f32, y: f32, z: f32) -> Self {
//         Self {
//             position: Vec3 { x, y, z },
//             ..Default::default()
//         }
//     }
// }

#[derive(Default, Debug, Clone, Copy)]
pub struct GeoSurface {
    pub start_index: u32,
    pub count: u32,
}

struct GpuMeshBuffers {
    index_buffer: AllocatedBuffer,
    _vertex_buffer: AllocatedBuffer,
    vertex_buffer_address: vk::DeviceAddress,
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

/// Loads the glTF file and uploads it to GPU memory
pub fn load_gltf_meshes(
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
                    if let Some(iter) = reader.read_indices() {
                        indices.extend(iter.into_u32().map(|i| i + initial_vtx as u32));
                    }

                    if let Some(pos_accessor) = primitive.get(&gltf::Semantic::Positions) {
                        vertices.reserve(pos_accessor.count());
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
