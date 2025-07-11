use std::{collections::HashMap, iter::zip, path::Path, rc::Rc};

use crate::{
    maths::ColorF32,
    scene::{Bounds, GeoSurface, MeshAsset, Texture, Vertex},
};
use glam::{Mat4, Vec3, Vec4, vec4};
use gltf::{Document, buffer};

/// Override colors with normal value
const OVERRIDE_COLORS: bool = false;

// TODO: better error handling
pub fn import_mesh_and_diffuse<P: AsRef<Path>>(path: P) -> MeshAsset {
    println!("Loading glTF : {}", path.as_ref().to_string_lossy());

    let (document, buffers, mut images_data) = gltf::import(path).unwrap();
    let (materials_vec, materials) = load_materials(&document);

    let (meshes_vec, meshes) = load_meshes(&document, buffers, materials_vec);

    todo!();
}

fn load_materials(document: &Document) -> (Vec<ColorF32>, HashMap<String, ColorF32>) {
    let mut materials = HashMap::new();
    let mut materials_vec = Vec::with_capacity(document.materials().count());
    materials_vec.extend(document.materials().map(|mat| {
        let new_mat = ColorF32::from_rgba(mat.pbr_metallic_roughness().base_color_factor());

        if let Some(name) = mat.name() {
            materials.insert(name.into(), new_mat);
        }

        new_mat
    }));

    (materials_vec, materials)
}

fn load_meshes(
    document: &Document,
    buffers: Vec<buffer::Data>,
    materials_vec: Vec<ColorF32>,
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
                        let start_index = indices.len();
                        let count = index_accessor.count();

                        let initial_vtx = vertices.len();
                        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                        indices.reserve(count);
                        if let Some(iter) = reader.read_indices() {
                            indices.extend(iter.into_u32().map(|i| i as usize + initial_vtx));
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

                        GeoSurface::new(
                            &vertices[..],
                            &indices[..],
                            start_index,
                            count,
                            materials_vec[primitive.material().index().unwrap_or(0)],
                        )
                    })
                    .collect();

                let mesh_buffers =
                    GpuMeshBuffers::new(device, commands, &indices[..], &vertices[..]);
                let new_mesh = Rc::new(MeshAsset::new(&vertices[..], &indices[..], surfaces));
                todo!("mesh.name().map(String::from),");

                if let Some(name) = mesh.name().map(String::from) {
                    meshes.insert(name, new_mesh.clone());
                }

                new_mesh
            })
            .collect()
    };

    (meshes_vec, meshes)
}
