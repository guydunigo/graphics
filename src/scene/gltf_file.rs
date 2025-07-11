use std::{cell::RefCell, collections::HashMap, iter::zip, path::Path, rc::Rc, time::Instant};

use crate::{
    maths::ColorF32,
    scene::{GeoSurface, MeshAsset, Node, Scene, Texture, Vertex},
};
use glam::{Mat4, Vec3, Vec4};
use gltf::{Document, buffer};

// TODO: better error handling
pub fn import_mesh_and_diffuse<P: AsRef<Path>>(path: P) -> Scene {
    let t0 = Instant::now();
    println!("Loading glTF : {}", path.as_ref().to_string_lossy());

    let t = Instant::now();
    let gltf::Gltf { document, blob } = gltf::Gltf::open(&path).unwrap();
    println!(" - Document loaded in : {}μs", t.elapsed().as_micros());

    let t = Instant::now();
    let buffers = gltf::import_buffers(&document, Some(path.as_ref()), blob).unwrap();
    println!(" - Buffers loaded in : {}μs", t.elapsed().as_micros());

    // let (document, buffers, _) = gltf::import(path).unwrap();

    let t = Instant::now();
    let (materials_vec, _materials) = load_materials(&document);
    println!(" - Materials loaded in : {}μs", t.elapsed().as_micros());

    let t = Instant::now();
    let (meshes_vec, _meshes) = load_meshes(&document, buffers, materials_vec);
    println!(" - Meshes loaded in : {}μs", t.elapsed().as_micros());

    let t = Instant::now();
    let (top_nodes, nodes) = load_nodes(&document, &meshes_vec[..]);
    println!(" - Nodes loaded in : {}μs", t.elapsed().as_micros());

    let t = Instant::now();
    let scene = Scene::new(nodes, top_nodes);
    println!(" - Remainder loaded in : {}μs", t.elapsed().as_micros());
    println!(" = Total : {}μs", t0.elapsed().as_micros());

    scene
}

fn load_materials(document: &Document) -> (Vec<Texture>, HashMap<String, Texture>) {
    let mut materials = HashMap::new();
    let mut materials_vec = Vec::with_capacity(document.materials().count());
    materials_vec.extend(document.materials().map(|mat| {
        let new_mat = ColorF32::from_rgba(mat.pbr_metallic_roughness().base_color_factor());
        let new_mat = Texture::Color(new_mat.as_color_u32());

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
    materials_vec: Vec<Texture>,
) -> (Vec<Rc<MeshAsset>>, HashMap<String, Rc<MeshAsset>>) {
    let mut meshes = HashMap::new();
    let mut meshes_vec = Vec::with_capacity(document.meshes().count());
    meshes_vec.extend(document.meshes().map(|mesh| {
        let mut indices = Vec::new();
        let mut vertices = Vec::new();

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
                    iter.enumerate()
                        .for_each(|(i, n)| vertices[initial_vtx + i].normal = Vec3::from_array(n));
                }

                if let Some(iter) = reader.read_tex_coords(0) {
                    iter.into_f32().enumerate().for_each(|(i, c)| {
                        let v = &mut vertices[initial_vtx + i];
                        v.uv_x = c[0];
                        v.uv_y = c[1];
                    });
                }

                if let Some(iter) = reader.read_colors(0) {
                    iter.into_rgba_f32()
                        .enumerate()
                        .for_each(|(i, c)| vertices[initial_vtx + i].color = Vec4::from_array(c));
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

        let new_mesh = Rc::new(MeshAsset::new(vertices, indices, surfaces));
        if let Some(name) = mesh.name().map(String::from) {
            meshes.insert(name, new_mesh.clone());
        }

        new_mesh
    }));

    (meshes_vec, meshes)
}

fn load_nodes(
    document: &Document,
    meshes_vec: &[Rc<MeshAsset>],
) -> (Vec<Rc<RefCell<Node>>>, HashMap<String, Rc<RefCell<Node>>>) {
    let mut nodes = HashMap::new();
    let mut nodes_vec: Vec<_> = Vec::with_capacity(document.nodes().count());
    nodes_vec.extend(document.nodes().map(|node| {
        let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
        let mut new_node = Node::new(local_transform);

        if let Some(mesh) = node.mesh() {
            new_node.mesh = Some(meshes_vec[mesh.index()].clone());
        };

        let new_node = Rc::new(RefCell::new(new_node));

        if let Some(name) = node.name() {
            nodes.insert(name.into(), new_node.clone());
        }

        new_node
    }));

    // Parent-children
    zip(document.nodes(), nodes_vec.iter()).for_each(|(node, new_node)| {
        let mut new_node_mut = new_node.borrow_mut();
        new_node_mut.children.reserve(node.children().count());
        new_node_mut.children.extend(node.children().map(|c| {
            let new_c = &nodes_vec[c.index()];
            // We hope a node can't be its own parent, otherwise borrow_mut would panic.
            new_c.borrow_mut().parent = Rc::downgrade(new_node);
            new_c.clone()
        }));
    });

    // Searching for parent-less nodes
    let top_nodes = nodes_vec
        .iter()
        .filter(|n| n.borrow().parent.strong_count() == 0)
        .cloned()
        .inspect(|n| n.borrow_mut().refresh_transform(&Mat4::IDENTITY))
        .collect();

    (top_nodes, nodes)
}
