use std::{
    cell::RefCell,
    collections::HashMap,
    iter::zip,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
};

use ::image::{DynamicImage, GrayImage, RgbImage};
use ash::{Device, vk};
use glam::{Mat4, Vec3, Vec4, vec4};
use gltf::{
    Document, buffer, image,
    material::AlphaMode,
    texture::{MagFilter, MinFilter},
};
use vk_mem::Allocator;

use super::{
    allocated::{AllocatedBuffer, AllocatedImage, MyMemoryUsage},
    commands::VulkanCommands,
    descriptors::DescriptorAllocatorGrowable,
    scene::{GeoSurface, GpuMeshBuffers, MeshAsset, MeshNode, Node, NodeData, Renderable},
    textures::{MaterialConstants, MaterialInstance, MaterialPass, MaterialResources, Textures},
};
use crate::scene::{Bounds, Vertex};

/// Override colors with normal value
const OVERRIDE_COLORS: bool = false;

/*
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
*/

pub struct LoadedGLTF {
    device_copy: Rc<Device>,

    meshes: HashMap<String, Rc<MeshAsset>>,
    nodes: HashMap<String, Rc<RefCell<dyn Node>>>,
    // images: HashMap<String, Rc<AllocatedImage>>,
    images: Vec<Rc<AllocatedImage>>,
    materials: HashMap<String, Rc<MaterialInstance>>,

    top_nodes: Vec<Rc<RefCell<dyn Node>>>,

    samplers: Vec<vk::Sampler>,

    descriptor_pool: DescriptorAllocatorGrowable,

    material_data_buffer: AllocatedBuffer,
}

impl Drop for LoadedGLTF {
    fn drop(&mut self) {
        #[cfg(feature = "vulkan_dbg_mem")]
        println!("drop LoadedGLTF");
        unsafe {
            self.samplers
                .drain(..)
                .for_each(|s| self.device_copy.destroy_sampler(s, None));
        }
    }
}

impl Renderable for LoadedGLTF {
    fn draw(&self, top_mat: &glam::Mat4, ctx: &mut super::scene::DrawContext) {
        self.top_nodes
            .iter()
            .for_each(|n| n.borrow().draw(top_mat, ctx));
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

        let (document, buffers, mut images_data) = gltf::import(path).unwrap();

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
        // TODO check sizes
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

        // TODO: parallel loading/converting images ?

        // let mut images = HashMap::new();
        // let images_vec: Vec<Rc<AllocatedImage>> = zip(images_data.drain(..), document.images())
        //     .enumerate()
        //     .map(|(i, (img_data, img))| {
        //         load_image(commands, device.clone(), allocator.clone(), img_data)
        //             .map(Rc::new)
        //             .inspect(|img_data| {
        //                 if let Some(name) = img.name() {
        //                     images.insert(name.into(), img_data.clone());
        //                 }
        //             })
        //             .unwrap_or_else(|err| {
        //                 eprintln!("Failed to load image #{i}, using default : {err}");
        //                 textures.error_checkerboard.clone()
        //             })
        //     })
        //     .collect();
        let images: Vec<Rc<AllocatedImage>> = images_data
            .drain(..)
            .enumerate()
            .map(|(i, img_data)| {
                load_image(commands, device.clone(), allocator.clone(), img_data)
                    .map(Rc::new)
                    .unwrap_or_else(|err| {
                        eprintln!("Failed to load image #{i}, using default : {err}");
                        textures.error_checkerboard.clone()
                    })
            })
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

        let (meshes_vec, meshes) =
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

                new_node.borrow_mut().node_data_mut().local_transform =
                    Mat4::from_cols_array_2d(&node.transform().matrix());

                new_node
            })
            .collect();

        // Parent-children
        zip(document.nodes(), nodes_vec.iter()).for_each(|(node, new_node)| {
            new_node
                .borrow_mut()
                .node_data_mut()
                .children
                .extend(node.children().map(|c| {
                    let new_c = &nodes_vec[c.index()];
                    // We hope a node can't be its own parent, otherwise borrow_mut would panic.
                    new_c.borrow_mut().node_data_mut().parent = Rc::downgrade(new_node);
                    new_c.clone()
                }));
        });

        // Searching for parent-less nodes
        let top_nodes = nodes_vec
            .iter()
            .filter(|n| n.borrow().node_data().parent.strong_count() == 0)
            .cloned()
            .inspect(|n| n.borrow_mut().refresh_transform(&Mat4::IDENTITY))
            .collect();

        Self {
            device_copy: device.clone(),
            meshes,
            nodes,
            images,
            materials,
            top_nodes,
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
    // TODO: alignment ?
    let scene_material_constants: &mut [MaterialConstants] =
        unsafe { std::slice::from_raw_parts_mut(data as *mut MaterialConstants, materials_len) };

    let mut materials = HashMap::new();
    let materials_vec: Vec<Rc<MaterialInstance>> =
        zip(scene_material_constants.iter_mut(), document.materials())
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
                    data_buffer_offset: (buf_slot as *const MaterialConstants) as u32 - data as u32,
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
    buffers: Vec<buffer::Data>,
    materials_vec: Vec<Rc<MaterialInstance>>,
) -> (Vec<Rc<MeshAsset>>, HashMap<String, Rc<MeshAsset>>) {
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
                        let start_index = indices.len() as u32;
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

                        if OVERRIDE_COLORS {
                            vertices[initial_vtx..]
                                .iter_mut()
                                .for_each(|v| v.color = v.normal.extend(1.));
                        } else if let Some(iter) = reader.read_colors(0) {
                            iter.into_rgba_f32().enumerate().for_each(|(i, c)| {
                                vertices[initial_vtx + i].color = Vec4::from_array(c)
                            });
                        }

                        GeoSurface {
                            start_index,
                            count: count as u32,
                            material: materials_vec[primitive.material().index().unwrap_or(0)]
                                .clone(),

                            bounds: Bounds::from_vertices(&vertices[initial_vtx..]),
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

    (meshes_vec, meshes)
}

fn load_image(
    commands: &VulkanCommands,
    device: Rc<Device>,
    allocator: Arc<Mutex<Allocator>>,
    image: image::Data,
) -> Result<AllocatedImage, String> {
    let extent = vk::Extent3D {
        width: image.width,
        height: image.height,
        depth: 1,
    };

    let (format, data) = match image.format {
        image::Format::R8 => {
            println!(
                "Image has format {:?}, converting it to R8G8B8A8.",
                image.format
            );
            let pixels = DynamicImage::ImageLuma8(
                GrayImage::from_raw(image.width, image.height, image.pixels).unwrap(),
            )
            .to_rgba8()
            .into_raw();

            (vk::Format::R8G8B8A8_UNORM, pixels)
        }
        // Not supported for other operations, needs converting to RGBA8
        image::Format::R8G8B8 => {
            let pixels = DynamicImage::ImageRgb8(
                RgbImage::from_raw(image.width, image.height, image.pixels).unwrap(),
            )
            .to_rgba8()
            .into_raw();

            (vk::Format::R8G8B8A8_UNORM, pixels)
        }
        image::Format::R8G8B8A8 => (vk::Format::R8G8B8A8_UNORM, image.pixels),
        // image::Format::R8 => vk::Format::R8_UNORM,
        // image::Format::R8G8 => vk::Format::R8G8_UNORM,
        // image::Format::R16 => vk::Format::R16_UNORM,
        // image::Format::R16G16 => vk::Format::R16G16_UNORM,
        // image::Format::R16G16B16 => vk::Format::R16G16B16_UNORM,
        // image::Format::R16G16B16A16 => vk::Format::R16G16B16A16_UNORM,
        // image::Format::R32G32B32FLOAT => vk::Format::R32G32B32_SFLOAT,
        // image::Format::R32G32B32A32FLOAT => vk::Format::R32G32B32A32_SFLOAT,
        _ => return Err(format!("Unsupported image format : {:?} !", image.format)),
    };

    /*
    // From [`gltf::image::Data`] : file:///home/Guillaume.Goni/workspace/rust/graphics/target/doc/src/gltf/image.rs.html#85-97
    // Conversion from `image` crate format :
    let format = match image {
        DynamicImage::ImageLuma8(_) => Format::R8,
        DynamicImage::ImageLumaA8(_) => Format::R8G8,
        DynamicImage::ImageRgb8(_) => Format::R8G8B8,
        DynamicImage::ImageRgba8(_) => Format::R8G8B8A8,
        DynamicImage::ImageLuma16(_) => Format::R16,
        DynamicImage::ImageLumaA16(_) => Format::R16G16,
        DynamicImage::ImageRgb16(_) => Format::R16G16B16,
        DynamicImage::ImageRgba16(_) => Format::R16G16B16A16,
        DynamicImage::ImageRgb32F(_) => Format::R32G32B32FLOAT,
        DynamicImage::ImageRgba32F(_) => Format::R32G32B32A32FLOAT,
        image => return Err(Error::UnsupportedImageFormat(image)),
    };
    */

    Ok(AllocatedImage::new_and_upload(
        commands,
        device,
        allocator,
        extent,
        format,
        vk::ImageUsageFlags::SAMPLED,
        true,
        &data[..],
    ))
}
