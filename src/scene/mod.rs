/// Describing the world
mod camera;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub use camera::Camera;
mod mesh;
use glam::{Mat4, Vec3, vec3};
pub use mesh::*;
mod mesh_library;
pub mod obj_file;

pub const DEFAULT_BACKGROUND_COLOR: u32 = 0xff181818;

pub struct World {
    pub scene: Scene,
    // TODO: copy vulkan camera and world info
    pub camera: Camera,
    pub sun_direction: Vec3,
}

impl Default for World {
    fn default() -> Self {
        World {
            scene: mesh_library::base_scene(),
            camera: Default::default(),
            sun_direction: vec3(-1., -1., -1.).normalize(),
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

pub struct Scene {
    // meshes: HashMap<String, Rc<MeshAsset>>,
    named_nodes: HashMap<String, Rc<RefCell<Node>>>,

    top_nodes: Vec<Rc<RefCell<Node>>>,
}

impl Scene {
    pub fn new(
        named_nodes: HashMap<String, Rc<RefCell<Node>>>,
        top_nodes: Vec<Rc<RefCell<Node>>>,
    ) -> Self {
        // Update world transform infos to all nodes.
        top_nodes
            .iter()
            .for_each(|n| n.borrow_mut().refresh_transform(&Mat4::IDENTITY));

        Scene {
            named_nodes,
            top_nodes,
        }
    }

    pub fn top_nodes(&self) -> &[Rc<RefCell<Node>>] {
        &self.top_nodes
    }

    pub fn get_named_node(&self, name: &str) -> Option<&Rc<RefCell<Node>>> {
        self.named_nodes.get(name)
    }
}
