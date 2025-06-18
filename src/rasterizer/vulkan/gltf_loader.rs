use std::{collections::HashMap, path::Path, rc::Rc};

use ash::{Device, vk};
use glam::{Vec3, Vec4};

use super::{
    allocated::{AllocatedBuffer, AllocatedImage},
    commands::VulkanCommands,
    descriptors::DescriptorAllocatorGrowable,
    scene::{GeoSurface, GpuMeshBuffers, MeshAsset, Node, Renderable, Vertex},
    textures::MaterialInstance,
};

/// Override colors with normal value
const OVERRIDE_COLORS: bool = false;

/// Loads the glTF file and uploads it to GPU memory
pub fn load_gltf_meshes(
    device: &Device,
    commands: &VulkanCommands,
    default_material: Rc<MaterialInstance>,
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

            let name = mesh
                .name()
                .map(|n| n.to_string())
                .expect("Mesh with no name !");
            let surfaces = mesh
                .primitives()
                .filter_map(|p| p.indices().map(|i| (p, i)))
                .map(|(primitive, index_accessor)| {
                    let count = index_accessor.count();

                    let surface = GeoSurface {
                        start_index: indices.len() as u32,
                        count: count as u32,
                        material: default_material.clone(),
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

            if OVERRIDE_COLORS {
                vertices
                    .iter_mut()
                    .for_each(|v| v.color = v.normal.extend(1.));
            }

            let mesh_buffers = GpuMeshBuffers::new(device, commands, &indices[..], &vertices[..]);

            MeshAsset::new(name, surfaces, mesh_buffers)
        })
        .collect()
}

struct LoadedGLTF {
    meshes: HashMap<String, Rc<MeshAsset>>,
    nodes: HashMap<String, Rc<dyn Node>>,
    images: HashMap<String, AllocatedImage>,
    materials: HashMap<String, Rc<MaterialInstance>>,

    top_nodes: HashMap<String, Rc<dyn Node>>,

    samplers: Vec<vk::Sampler>,

    descriptor_pool: DescriptorAllocatorGrowable,

    material_data_buffer: AllocatedBuffer,
}

impl Renderable for LoadedGLTF {
    fn draw(&self, top_mat: &glam::Mat4, ctx: &mut super::scene::DrawContext) {
        todo!()
    }
}
