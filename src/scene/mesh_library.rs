use std::{cell::RefCell, collections::HashMap, iter::once, rc::Rc};

use glam::{Mat4, Quat, Vec3, vec3};
/// Set of constructor functions to get testing objects
use rand::RngCore;

use super::{GeoSurface, MeshAsset, Node, Scene, Texture, Vertex, obj_file};
use crate::maths::PI;

pub fn base_scene() -> Scene {
    let suzanne: Node = obj_file::import_mesh_and_diffuse(obj_file::SUZANNE_OBJ_PATH).into();
    let suzanne = Rc::new(RefCell::new(suzanne));

    let pyramid = Rc::new(RefCell::new(base_pyramid()));

    let top = Node::parent_of(
        vec![
            base_triangle(),
            floor(),
            back_wall(),
            left_wall(),
            right_wall(),
        ]
        .drain(..)
        .map(|n| Rc::new(RefCell::new(n)))
        .chain(once(suzanne.clone()))
        .chain(once(pyramid.clone()))
        .collect(),
    );

    let mut nodes = HashMap::new();
    nodes.insert("suzanne".to_string(), suzanne);
    nodes.insert("pyramid".to_string(), pyramid);

    Scene::new(nodes, vec![top])
}

fn base_triangle() -> Node {
    let vertices: Vec<_> = [vec3(0., 1., -2.), vec3(0., 0., 0.), vec3(0., 0., -4.)]
        .iter()
        .map(|p| Vertex {
            position: *p,
            ..Default::default()
        })
        .collect();
    let indices = vec![0, 1, 2];
    let surfaces = vec![GeoSurface::new(
        &vertices,
        &indices,
        0,
        indices.len(),
        Texture::VertexColor(0xffff0000, 0xff00ff00, 0xff0000ff),
    )];

    Node::new_mesh(
        Rc::new(MeshAsset::new(vertices, indices, surfaces)),
        Mat4::from_translation(vec3(0., 0., -10.)),
    )
}

fn base_pyramid() -> Node {
    let vertices: Vec<_> = [
        vec3(-1., -1., 0.),
        vec3(0., -1., 0.),
        vec3(0., 0., 9.),
        vec3(1., -1., 0.),
        vec3(-1., 1., 0.),
        vec3(0., 1., 0.),
        vec3(1., 1., 0.),
        vec3(-1., 0., 0.),
        vec3(1., 0., 0.),
        vec3(-2., -0.5, 0.),
        vec3(0., -0.5, 4.),
        vec3(-2., 0.5, 0.),
        vec3(0., 0.5, 4.),
        vec3(-0.3, -0.3, 7.),
        vec3(0.3, -0.3, 7.),
        vec3(-0.3, 0.3, 7.),
        vec3(0.3, 0.3, 7.),
    ]
    .iter()
    .map(|p| Vertex {
        position: *p,
        ..Default::default()
    })
    .collect();
    #[rustfmt::skip]
    let indices = vec![
        0, 1, 2,
        1, 3, 2,

        4, 2, 5,
        2, 6, 5,

        0, 2, 7,
        4, 7, 2,

        8, 2, 3,
        2, 8, 6,

        9, 10, 11,
        10, 12, 11,

        13, 14, 15,
        14, 16, 15,
    ];
    let surfaces = vec![
        GeoSurface::new(&vertices, &indices, 0, 6, Texture::Color(0xffff0000)),
        GeoSurface::new(&vertices, &indices, 6, 6, Texture::Color(0xff0000ff)),
        GeoSurface::new(&vertices, &indices, 12, 6, Texture::Color(0xff00ff00)),
        GeoSurface::new(&vertices, &indices, 18, 6, Texture::Color(0xffffff00)),
        GeoSurface::new(&vertices, &indices, 24, 6, Texture::Color(0xff00ffff)),
        GeoSurface::new(&vertices, &indices, 30, 6, Texture::Color(0xffff00ff)),
    ];

    Node::new_mesh(
        Rc::new(MeshAsset::new(vertices, indices, surfaces)),
        Mat4::from_scale_rotation_translation(
            Vec3::splat(0.7),
            Quat::from_rotation_z(-PI / 3.),
            vec3(4., 1., -19.),
        ),
    )
}

fn triangles_plane_mesh(color_mask: u32) -> MeshAsset {
    const RANGE: i32 = 10;
    let vertices: Vec<_> = (-RANGE..=RANGE)
        .flat_map(|x| {
            (-RANGE..=RANGE).map(move |z| Vertex {
                position: vec3(x as f32, 0., z as f32),
                ..Default::default()
            })
        })
        .collect();
    // Skip last column/lign as they won't start triangles
    const RANGE_2: i32 = RANGE * 2;
    let indices: Vec<_> = (0..RANGE_2)
        .flat_map(|x| {
            (0..RANGE_2).flat_map(move |z| {
                [
                    x * (RANGE_2 + 1) + z,
                    x * (RANGE_2 + 1) + z + 1,
                    (x + 1) * (RANGE_2 + 1) + z + 1,
                ]
            })
        })
        .map(|i| i as usize)
        .collect();
    let surfaces: Vec<_> = (0..indices.len())
        .step_by(3)
        .map(|i| {
            GeoSurface::new(
                &vertices,
                &indices,
                i,
                3,
                Texture::Color(rand::rng().next_u32() & color_mask),
            )
        })
        .collect();

    MeshAsset::new(vertices, indices, surfaces)
}

fn triangles_plane(color_mask: u32, pos: Vec3, rot: Quat, scale: f32) -> Node {
    Node::new_mesh(
        Rc::new(triangles_plane_mesh(color_mask)),
        Mat4::from_scale_rotation_translation(Vec3::splat(scale), rot, pos),
    )
}

fn floor() -> Node {
    triangles_plane(0xff00ffff, vec3(0., -10., 0.), Quat::default(), 5.)
}

fn back_wall() -> Node {
    triangles_plane(
        0xffffff00,
        vec3(0., 0., -30.),
        Quat::from_rotation_z(-PI / 2.) * Quat::from_rotation_x(PI / 2.),
        1.,
    )
}

fn left_wall() -> Node {
    triangles_plane(
        0xffff00ff,
        vec3(-10., 0., 0.),
        Quat::from_rotation_z(-PI / 2.),
        1.,
    )
}

fn right_wall() -> Node {
    triangles_plane(
        0xff0ff00f,
        vec3(10., 0., 0.),
        Quat::from_rotation_z(PI / 2.),
        1.,
    )
}
