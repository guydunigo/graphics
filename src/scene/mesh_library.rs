use std::{cell::RefCell, collections::HashMap, iter::once, rc::Rc};

use glam::{Mat4, Quat, Vec3, vec3};
/// Set of constructor functions to get testing objects
use rand::RngCore;

use super::{GeoSurface, MeshAsset, Node, Scene, Texture, Vertex, obj_file};
use crate::maths::PI;

pub fn base_scene() -> Scene {
    let suzanne: Node = obj_file::import_mesh_and_diffuse(obj_file::SUZANNE_OBJ_PATH).into();
    let suzanne = Rc::new(RefCell::new(suzanne));

    let top = Node::parent_of(
        vec![
            base_triangle(),
            base_pyramid(),
            floor(),
            back_wall(),
            left_wall(),
            right_wall(),
        ]
        .drain(..)
        .map(|n| Rc::new(RefCell::new(n)))
        .chain(once(suzanne.clone()))
        .collect(),
    );

    let mut nodes = HashMap::new();
    nodes.insert("suzanne".to_string(), suzanne);

    Scene {
        nodes,
        top_nodes: vec![top],
    }
}

fn base_triangle() -> Node {
    let vertices: Vec<_> = [vec3(0., 1., -2.), vec3(0., 0., 0.), vec3(0., 0., -4.)]
        .iter()
        .map(|p| Vertex { position: *p })
        .collect();
    let indices = vec![0, 1, 2];
    let surfaces = vec![GeoSurface::new(
        &vertices,
        0,
        indices.len(),
        Texture::VertexColor(0xffff0000, 0xff00ff00, 0xff0000ff),
    )];

    Node {
        parent: Default::default(),
        children: Default::default(),

        local_transform: Mat4::from_translation(vec3(0., 0., -10.)),
        world_transform: Default::default(),

        mesh: Some(Rc::new(MeshAsset {
            vertices,
            indices,
            surfaces,
        })),
    }
}

fn base_pyramid() -> Node {
    let vertices: Vec<_> = [
        vec3(-1., -1., 0.),
        vec3(0., -1., 0.),
        vec3(0., 0., 9.),
        vec3(0., -1., 0.),
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
    .map(|p| Vertex { position: *p })
    .collect();
    #[rustfmt::skip]
    let indices = vec![
        0, 1, 2,
        3, 4, 2,
        5, 2, 6,
        2, 7, 6,
        0, 2, 8,
        5, 8, 2,
        9, 2, 4,
        2, 9, 7,
        10, 11, 12,
        11, 13, 12,
        14, 15, 16,
        15, 17, 16,
    ];
    let surfaces = vec![
        GeoSurface::new(&vertices, 0, 2, Texture::Color(0xffff0000)),
        GeoSurface::new(&vertices, 2, 2, Texture::Color(0xff0000ff)),
        GeoSurface::new(&vertices, 4, 2, Texture::Color(0xff00ff00)),
        GeoSurface::new(&vertices, 6, 2, Texture::Color(0xffffff00)),
        GeoSurface::new(&vertices, 8, 2, Texture::Color(0xff00ffff)),
        GeoSurface::new(&vertices, 10, 2, Texture::Color(0xffff00ff)),
    ];

    Node {
        parent: Default::default(),
        children: Default::default(),

        local_transform: Mat4::from_scale_rotation_translation(
            Vec3::splat(0.7),
            Quat::from_rotation_z(-PI / 3.),
            vec3(4., 1., -19.),
        ),

        world_transform: Default::default(),

        mesh: Some(Rc::new(MeshAsset {
            vertices,
            indices,
            surfaces,
        })),
    }
}

fn triangles_plane_mesh(color_mask: u32) -> MeshAsset {
    const RANGE: i32 = 10;
    let vertices: Vec<_> = (-RANGE..=RANGE)
        .flat_map(|x| {
            (-RANGE..=RANGE).map(move |z| Vertex {
                position: vec3(x as f32, 0., z as f32),
            })
        })
        .collect();
    // Skip last column/lign as they won't start triangles
    const RANGE_2: i32 = RANGE * 2;
    let indices: Vec<_> = (0..RANGE_2)
        .flat_map(|x| {
            (0..RANGE_2).flat_map(move |z| {
                // Ugly, but we need an owned iterator...
                once(x * RANGE_2 + z)
                    .chain(once((x + 1) * RANGE_2 + z + 1))
                    .chain(once((x + 1) * RANGE_2 + z))
                    .map(|i| i as usize)
            })
        })
        .collect();
    let surfaces = (0..indices.len() / 3)
        .map(|i| {
            GeoSurface::new(
                &vertices[i..=i],
                i,
                1,
                Texture::Color(rand::rng().next_u32() & color_mask),
            )
        })
        .collect();

    MeshAsset {
        vertices,
        indices,
        surfaces,
    }
}

fn triangles_plane(color_mask: u32, pos: Vec3, rot: Quat, scale: f32) -> Node {
    Node {
        parent: Default::default(),
        children: Default::default(),
        local_transform: Mat4::from_scale_rotation_translation(Vec3::splat(scale), rot, pos),
        world_transform: Default::default(),
        mesh: Some(Rc::new(triangles_plane_mesh(color_mask))),
    }
}

fn floor() -> Node {
    triangles_plane(0xff00ffff, vec3(0., -10., 0.), Quat::default(), 5.)
}

fn back_wall() -> Node {
    triangles_plane(
        0xffffff00,
        vec3(0., 0., -30.),
        Quat::from_rotation_x(PI / 2.),
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
