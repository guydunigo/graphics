/// Describing the world
mod camera;
use std::time::Instant;

mod scene;
pub use camera::Camera;
pub use scene::Scene;
mod mesh;
use glam::{Mat4, Vec3, Vec4Swizzles, vec3};
pub use mesh::*;
use winit::dpi::PhysicalSize;

use crate::scene::scene::SceneStandIn;

pub mod gltf_file;
mod mesh_library;
pub mod obj_file;

pub const DEFAULT_BACKGROUND_COLOR: u32 = 0xff181818;

pub struct World {
    pub scene: SceneStandIn,
    // TODO: copy vulkan world info
    pub camera: Camera,
    pub sun_direction: Vec3,
}

impl Default for World {
    fn default() -> Self {
        let t = Instant::now();
        let w = World {
            scene: SceneStandIn::new(
                "resources/structure.glb",
                gltf_file::import_mesh_and_diffuse,
            ),
            // scene: SceneStandIn::new_ready(mesh_library::base_scene()),
            // TODO: scene: mesh_library::base_scene(),
            camera: Default::default(),
            sun_direction: vec3(-1., -1., -1.).normalize(),
        };
        println!("World loaded in : {}μs", t.elapsed().as_micros());
        w
    }
}

const DEFAULT_COLOR: u32 = 0xff999999;

#[derive(Debug, Clone, Copy)]
pub enum Texture {
    /// A simple color for the whole triangle
    Color(u32),
    /// A color per vertex in the same order :
    VertexColor(u32, u32, u32),
    // Texture, // TODO
}

impl Default for Texture {
    fn default() -> Self {
        Self::Color(DEFAULT_COLOR)
    }
}

pub fn to_cam_tr(camera: &Camera, world_transform: &Mat4) -> Mat4 {
    camera.view_mat() * world_transform
}

pub fn local_to_clipspace(
    camera: &Camera,
    to_cam_tr: &Mat4,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
    p: &Vec3,
) -> Vec3 {
    let mut p = (to_cam_tr * p.extend(1.)).xyz();

    // Screen space : perspective correct
    if p.z < -0.001 {
        p.x *= camera.z_near / -p.z;
        p.y *= camera.z_near / -p.z;
    } else {
        // TODO: 0 divide getting too near the camera and reversing problem behind...
        p.x *= camera.z_near / 0.1;
        p.y *= camera.z_near / 0.1;
    };
    p.z = -p.z;

    // Near-Clipping-Plane
    // [-1,1]
    p.x /= camera.canvas_side;
    p.y /= camera.canvas_side;

    if size.width > size.height {
        p.x /= ratio_w_h;
    } else {
        p.y *= ratio_w_h;
    }

    p
}

pub fn to_raster(
    p_world: Vec3,
    cam: &Camera,
    to_cam_tr: &Mat4,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
) -> Vec3 {
    let mut p = local_to_clipspace(cam, to_cam_tr, size, ratio_w_h, &p_world);

    // Raster space
    // [0,1] -> [0,size]
    p.x = (p.x + 1.) / 2. * (size.width as f32);
    p.y = (1. - p.y) / 2. * (size.height as f32);

    p
}

pub fn world_to_raster(
    p_world: Vec3,
    cam: &Camera,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
) -> Vec3 {
    to_raster(p_world, cam, &cam.view_mat(), size, ratio_w_h)
}
