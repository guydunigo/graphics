/// Describing the world
mod camera;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

pub use camera::Camera;
mod mesh;
use glam::Mat4;
pub use mesh::*;
mod mesh_library;
pub mod obj_file;

use crate::maths::Vec3f;

pub const DEFAULT_BACKGROUND_COLOR: u32 = 0xff181818;

pub struct World {
    pub meshes: Vec<MeshAsset>,
    // TODO: copy vulkan camera and world info
    pub camera: Camera,
    pub sun_direction: Vec3f,
}

impl Default for World {
    fn default() -> Self {
        World {
            meshes: vec![
                mesh_library::base_triangle(),
                mesh_library::base_pyramid(),
                obj_file::import_mesh_and_diffuse(obj_file::SUZANNE_OBJ_PATH),
                mesh_library::floor(),
                mesh_library::back_wall(),
                mesh_library::left_wall(),
                mesh_library::right_wall(),
            ],
            camera: Default::default(),
            sun_direction: Vec3f::new(-1., -1., -1.).normalize(),
        }
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

pub struct Node {
    /// If there is no parent or it was destroyed, weak won't upgrade.
    pub parent: Weak<RefCell<Node>>,
    pub children: Vec<Rc<RefCell<Node>>>,
    pub local_transform: Mat4,
    /// Cache :
    world_transform: Mat4,
    mesh: Option<Rc<MeshAsset>>,
}

pub struct Scene {
    // meshes: HashMap<String, Rc<MeshAsset>>,
    nodes: HashMap<String, Rc<RefCell<Node>>>,

    top_nodes: Vec<Rc<RefCell<Node>>>,
}
