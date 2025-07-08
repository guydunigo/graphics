use glam::vec3;
/// Set of constructor functions to get testing objects
use rand::RngCore;

use super::{Bounds, GeoSurface, MeshAsset, Texture, Vertex};
use crate::maths::{PI, Rotation, Vec3f};

pub fn base_triangle() -> MeshAsset {
    let vertices = vec![
        Vertex {
            position: vec3(0., 1., -2.),
        },
        Vertex {
            position: vec3(0., 0., 0.),
        },
        Vertex {
            position: vec3(0., 0., -4.),
        },
    ];
    let indices = vec![0, 1, 2];
    let surfaces = vec![GeoSurface {
        start_index: 0,
        count: 3,
        material: Texture::VertexColor(0xffff0000, 0xff00ff00, 0xff0000ff),
        bounds: Bounds::new(&mut vertices.iter()),
    }];
    todo!("with_translation_to(Vec3f::new(0., 0., -10.))");
    MeshAsset {
        vertices,
        indices,
        surfaces,
    }
}

pub fn base_pyramid() -> MeshAsset {
    Mesh {
        triangles: vec![
            Triangle::new(
                Vec3f::new(-1., -1., 0.),
                Vec3f::new(0., -1., 0.),
                Vec3f::new(0., 0., 9.),
                Texture::Color(0xffff0000),
            ),
            Triangle::new(
                Vec3f::new(0., -1., 0.),
                Vec3f::new(1., -1., 0.),
                Vec3f::new(0., 0., 9.),
                Texture::Color(0xffff0000),
            ),
            Triangle::new(
                Vec3f::new(-1., 1., 0.),
                Vec3f::new(0., 0., 9.),
                Vec3f::new(0., 1., 0.),
                Texture::Color(0xff0000ff),
            ),
            Triangle::new(
                Vec3f::new(0., 0., 9.),
                Vec3f::new(1., 1., 0.),
                Vec3f::new(0., 1., 0.),
                Texture::Color(0xff0000ff),
            ),
            Triangle::new(
                Vec3f::new(-1., -1., 0.),
                Vec3f::new(0., 0., 9.),
                Vec3f::new(-1., 0., 0.),
                Texture::Color(0xff00ff00),
            ),
            Triangle::new(
                Vec3f::new(-1., 1., 0.),
                Vec3f::new(-1., 0., 0.),
                Vec3f::new(0., 0., 9.),
                Texture::Color(0xff00ff00),
            ),
            Triangle::new(
                Vec3f::new(1., 0., 0.),
                Vec3f::new(0., 0., 9.),
                Vec3f::new(1., -1., 0.),
                Texture::Color(0xffffff00),
            ),
            Triangle::new(
                Vec3f::new(0., 0., 9.),
                Vec3f::new(1., 0., 0.),
                Vec3f::new(1., 1., 0.),
                Texture::Color(0xffffff00),
            ),
            Triangle::new(
                Vec3f::new(-2., -0.5, 0.),
                Vec3f::new(0., -0.5, 4.),
                Vec3f::new(-2., 0.5, 0.),
                Texture::Color(0xff00ffff),
            ),
            Triangle::new(
                Vec3f::new(0., -0.5, 4.),
                Vec3f::new(0., 0.5, 4.),
                Vec3f::new(-2., 0.5, 0.),
                Texture::Color(0xff00ffff),
            ),
            Triangle::new(
                Vec3f::new(-0.3, -0.3, 7.),
                Vec3f::new(0.3, -0.3, 7.),
                Vec3f::new(-0.3, 0.3, 7.),
                Texture::Color(0xffff00ff),
            ),
            Triangle::new(
                Vec3f::new(0.3, -0.3, 7.),
                Vec3f::new(0.3, 0.3, 7.),
                Vec3f::new(-0.3, 0.3, 7.),
                Texture::Color(0xffff00ff),
            ),
        ],
        pos: Vec3f {
            x: 4.,
            y: 1.,
            z: -19.,
        },
        rot: Rotation::from_angles(0., 0., -PI / 3.),
        scale: 0.7,
    }
}

pub fn triangles_plane(color_mask: u32) -> Vec<Triangle> {
    const RANGE: i32 = 10;
    (-RANGE..RANGE)
        .flat_map(|x| {
            (-RANGE..RANGE)
                .map(move |z| {
                    (
                        Vec3f::new(x as f32, 0., z as f32),
                        rand::rng().next_u32() & color_mask,
                    )
                })
                .map(|(v, c)| {
                    Triangle::new(
                        v,
                        v + Vec3f::new(1., 0., 1.),
                        v + Vec3f::new(1., 0., 0.),
                        Texture::Color(c),
                    )
                })
        })
        .collect()
}

pub fn floor() -> MeshAsset {
    Mesh {
        triangles: triangles_plane(0xff00ffff),
        pos: Vec3f::new(0., -10., 0.),
        scale: 5.,
        ..Default::default()
    }
}

pub fn back_wall() -> MeshAsset {
    Mesh {
        triangles: triangles_plane(0xffffff00),
        pos: Vec3f::new(0., 0., -30.),
        scale: 1.,
        rot: Rotation::from_angles(PI / 2., 0., 0.),
    }
}

pub fn left_wall() -> MeshAsset {
    Mesh {
        triangles: triangles_plane(0xffff00ff),
        pos: Vec3f::new(-10., 0., 0.),
        scale: 1.,
        rot: Rotation::from_angles(0., 0., -PI / 2.),
    }
}

pub fn right_wall() -> MeshAsset {
    Mesh {
        triangles: triangles_plane(0xffff00ff),
        pos: Vec3f::new(10., 0., 0.),
        scale: 1.,
        rot: Rotation::from_angles(0., 0., PI / 2.),
    }
}
