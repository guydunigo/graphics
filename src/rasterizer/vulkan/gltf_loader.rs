use std::{
    cell::RefCell,
    collections::HashMap,
    mem,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
};

use ash::{Device, util::Align, vk};
use glam::{Vec3, Vec4, vec4};
use gltf::{
    Document,
    buffer::Data,
    material::AlphaMode,
    texture::{MagFilter, MinFilter},
};
use vk_mem::Allocator;

use super::{
    allocated::{AllocatedBuffer, AllocatedImage, MyMemoryUsage},
    commands::VulkanCommands,
    descriptors::DescriptorAllocatorGrowable,
    scene::{GeoSurface, GpuMeshBuffers, MeshAsset, MeshNode, Node, NodeData, Renderable, Vertex},
    textures::{MaterialConstants, MaterialInstance, MaterialPass, MaterialResources, Textures},
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

                    if let Some(iter) = reader.read_tex_coords(0) {
                        iter.into_f32().enumerate().for_each(|(i, c)| {
                            let v = &mut vertices[initial_vtx + i];
                            v.uv_x = c[0];
                            v.uv_y = c[1];
                        });
                    }

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

            MeshAsset::new(mesh.name().map(|n| n.to_string()), surfaces, mesh_buffers)
        })
        .collect()
}

struct LoadedGLTF {
    meshes: HashMap<String, Rc<MeshAsset>>,
    nodes: HashMap<String, Rc<RefCell<dyn Node>>>,
    images: HashMap<String, Rc<AllocatedImage>>,
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

impl LoadedGLTF {
    pub fn load(
        device: Rc<Device>,
        allocator: Arc<Mutex<Allocator>>,
        commands: &VulkanCommands,
        textures: &mut Textures,
        path: impl AsRef<Path>,
    ) -> Self {
        println!("Loading glTF : {}", path.as_ref().to_string_lossy());

        let (document, buffers, images) = gltf::import(path).unwrap();

        let samplers: Vec<vk::Sampler> = document
            .samplers()
            .map(|s| {
                let create_info = vk::SamplerCreateInfo::default()
                    .max_lod(vk::LOD_CLAMP_NONE)
                    .min_lod(0.)
                    .mag_filter(extract_mag_filter(s.mag_filter()))
                    .min_filter(extract_min_filter(s.min_filter()))
                    .mipmap_mode(extract_mipmap_mode(s.min_filter()));
                unsafe { device.create_sampler(&create_info, None).unwrap() }
            })
            .collect();

        // TODO: we can estimate closely the needs dependending on the file
        let sizes = [
            (vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 3.),
            (vk::DescriptorType::UNIFORM_BUFFER, 3.),
            (vk::DescriptorType::STORAGE_BUFFER, 1.),
        ];
        // TODO: count ? get len directly ? .as_json().materials.len()
        let materials_len = document.materials().count();
        let mut descriptor_pool =
            DescriptorAllocatorGrowable::new(device.clone(), materials_len as u32, &sizes[..]);

        // Chargement dans l'ordre des dépendences
        let images: Vec<Rc<AllocatedImage>> = images
            .iter()
            .map(|_| textures.error_checkerboard.clone())
            .collect();

        let (material_data_buffer, materials_vec, materials) = load_materials(
            allocator,
            textures,
            &document,
            &images[..],
            &samplers[..],
            materials_len,
            &mut descriptor_pool,
        );

        let (meshes, meshes_vec) =
            load_meshes(&device, commands, &document, buffers, materials_vec);

        let mut nodes = HashMap::new();
        let nodes_vec: Vec<Rc<RefCell<dyn Node>>> = document
            .nodes()
            .map(|node| {
                let new_node: Rc<RefCell<dyn Node>> = if let Some(mesh) = node.mesh() {
                    Rc::new(RefCell::new(MeshNode::from(
                        meshes_vec[mesh.index()].clone(),
                    )))
                } else {
                    Rc::new(RefCell::new(NodeData::default()))
                };

                if let Some(name) = node.name() {
                    nodes.insert(name.into(), new_node.clone());
                }

                todo!("visit");

                new_node
            })
            .collect();

        Self {
            meshes,
            nodes,
            images: todo!(),
            materials,
            top_nodes: todo!(),
            samplers,
            descriptor_pool,
            material_data_buffer,
        }
    }
}

fn extract_mag_filter(filter: Option<MagFilter>) -> vk::Filter {
    match filter {
        Some(MagFilter::Nearest) => vk::Filter::NEAREST,
        Some(MagFilter::Linear) => vk::Filter::LINEAR,
        None => vk::Filter::NEAREST,
    }
}

fn extract_min_filter(filter: Option<MinFilter>) -> vk::Filter {
    use MinFilter::*;
    match filter {
        Some(Nearest) | Some(NearestMipmapNearest) | Some(NearestMipmapLinear) => {
            vk::Filter::NEAREST
        }
        Some(Linear) | Some(LinearMipmapNearest) | Some(LinearMipmapLinear) => vk::Filter::LINEAR,
        None => vk::Filter::NEAREST,
    }
}

fn extract_mipmap_mode(filter: Option<MinFilter>) -> vk::SamplerMipmapMode {
    use MinFilter::*;
    match filter {
        Some(NearestMipmapNearest) | Some(LinearMipmapNearest) => vk::SamplerMipmapMode::NEAREST,
        Some(NearestMipmapLinear) | Some(LinearMipmapLinear) => vk::SamplerMipmapMode::LINEAR,
        _ => vk::SamplerMipmapMode::LINEAR,
    }
}

fn load_materials(
    allocator: Arc<Mutex<vk_mem::Allocator>>,
    textures: &mut Textures,
    document: &Document,
    images: &[Rc<AllocatedImage>],
    samplers: &[vk::Sampler],
    materials_len: usize,
    descriptor_pool: &mut DescriptorAllocatorGrowable,
) -> (
    AllocatedBuffer,
    Vec<Rc<MaterialInstance>>,
    HashMap<String, Rc<MaterialInstance>>,
) {
    let material_data_buffer_size = (size_of::<MaterialConstants>() * materials_len) as u64;
    let material_data_buffer = AllocatedBuffer::new(
        allocator,
        material_data_buffer_size,
        vk::BufferUsageFlags::UNIFORM_BUFFER,
        MyMemoryUsage::CpuToGpu,
    );

    let data = material_data_buffer.mapped_data();
    let mut scene_material_constants = unsafe {
        Align::new(
            data,
            mem::align_of::<MaterialConstants>() as _,
            material_data_buffer_size,
        )
    };

    let mut materials = HashMap::new();
    let materials_vec: Vec<Rc<MaterialInstance>> =
        std::iter::zip(scene_material_constants.iter_mut(), document.materials())
            .map(|(buf_slot, mat)| {
                let pbr_data = mat.pbr_metallic_roughness();
                *buf_slot = MaterialConstants {
                    color_factors: pbr_data.base_color_factor().into(),
                    metal_rough_factors: vec4(
                        pbr_data.metallic_factor(),
                        pbr_data.roughness_factor(),
                        0.,
                        0.,
                    ),
                };

                let pass_type = if mat.alpha_mode() == AlphaMode::Blend {
                    MaterialPass::Transparent
                } else {
                    MaterialPass::MainColor
                };

                let (color_img, color_sampler) = if let Some(bct) = pbr_data.base_color_texture() {
                    let texture = bct.texture();
                    (
                        &images[texture.source().index()],
                        samplers[texture.sampler().index().unwrap_or(0)],
                    )
                } else {
                    (&textures.white, textures.default_sampler_linear)
                };

                let material_resources = MaterialResources {
                    // TODO default values ?
                    color_img,
                    color_sampler,
                    metal_rough_img: &textures.white,
                    metal_rough_sampler: textures.default_sampler_linear,
                    data_buffer: material_data_buffer.buffer,
                    data_buffer_offset: dbg!(
                        (buf_slot as *const MaterialConstants) as u32 - data as u32
                    ),
                };

                let new_mat = textures.metal_rough_material.write_material(
                    pass_type,
                    &material_resources,
                    descriptor_pool,
                );

                let new_mat = Rc::new(new_mat);

                if let Some(name) = mat.name() {
                    materials.insert(name.into(), new_mat.clone());
                }

                new_mat
            })
            .collect();

    (material_data_buffer, materials_vec, materials)
}

fn load_meshes(
    device: &Device,
    commands: &VulkanCommands,
    document: &Document,
    buffers: Vec<Data>,
    materials_vec: Vec<Rc<MaterialInstance>>,
) -> (HashMap<String, Rc<MeshAsset>>, Vec<Rc<MeshAsset>>) {
    let mut meshes = HashMap::new();
    let meshes_vec = {
        // In common to prevent reallocating much
        let mut indices = Vec::new();
        let mut vertices = Vec::new();
        document
            .meshes()
            .map(|mesh| {
                indices.clear();
                vertices.clear();

                let surfaces = mesh
                    .primitives()
                    .filter_map(|p| p.indices().map(|i| (p, i)))
                    .map(|(primitive, index_accessor)| {
                        let count = index_accessor.count();

                        let initial_vtx = vertices.len();
                        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                        indices.reserve(count);
                        if let Some(iter) = reader.read_indices() {
                            indices.extend(iter.into_u32().map(|i| i + initial_vtx as u32));
                        }

                        if let Some(pos_accessor) = primitive.get(&gltf::Semantic::Positions) {
                            vertices.reserve(pos_accessor.count());
                            if let Some(iter) = reader.read_positions() {
                                vertices.extend(iter.map(|v| Vertex {
                                    position: Vec3::from_array(v),
                                    ..Default::default()
                                }));
                            }
                        }

                        if let Some(iter) = reader.read_normals() {
                            iter.enumerate().for_each(|(i, n)| {
                                vertices[initial_vtx + i].normal = Vec3::from_array(n)
                            });
                        }

                        if let Some(iter) = reader.read_tex_coords(0) {
                            iter.into_f32().enumerate().for_each(|(i, c)| {
                                let v = &mut vertices[initial_vtx + i];
                                v.uv_x = c[0];
                                v.uv_y = c[1];
                            });
                        }

                        if let Some(iter) = reader.read_colors(0) {
                            iter.into_rgba_f32().enumerate().for_each(|(i, c)| {
                                vertices[initial_vtx + i].color = Vec4::from_array(c)
                            });
                        }

                        GeoSurface {
                            start_index: indices.len() as u32,
                            count: count as u32,
                            material: materials_vec[primitive.material().index().unwrap_or(0)]
                                .clone(),
                        }
                    })
                    .collect();

                let mesh_buffers =
                    GpuMeshBuffers::new(device, commands, &indices[..], &vertices[..]);
                let new_mesh = Rc::new(MeshAsset::new(
                    mesh.name().map(String::from),
                    surfaces,
                    mesh_buffers,
                ));

                if let Some(name) = mesh.name().map(String::from) {
                    meshes.insert(name, new_mesh.clone());
                }

                new_mesh
            })
            .collect()
    };

    (meshes, meshes_vec)
}
