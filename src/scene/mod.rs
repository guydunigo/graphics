/// Describing the world
mod camera;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

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

pub struct Node {
    /// If there is no parent or it was destroyed, weak won't upgrade.
    pub parent: Weak<RefCell<Node>>,
    pub children: Vec<Rc<RefCell<Node>>>,

    pub local_transform: Mat4,
    /// Cache :
    world_transform: Mat4,

    /// Actual mesh if any at this node
    mesh: Option<Rc<MeshAsset>>,
}

impl Node {
    pub fn parent_of(children: Vec<Rc<RefCell<Node>>>) -> Rc<RefCell<Self>> {
        Rc::new_cyclic(|f| {
            children
                .iter()
                .for_each(|c| c.borrow_mut().parent = f.clone());
            let node = Node {
                parent: Default::default(),
                children,

                local_transform: Default::default(),
                world_transform: Default::default(),

                mesh: None,
            };
            RefCell::new(node)
        })
    }
}

impl From<MeshAsset> for Node {
    fn from(value: MeshAsset) -> Self {
        Node {
            parent: Default::default(),
            children: Default::default(),

            local_transform: Default::default(),
            world_transform: Default::default(),

            mesh: Some(Rc::new(value)),
        }
    }
}

pub struct Scene {
    // meshes: HashMap<String, Rc<MeshAsset>>,
    nodes: HashMap<String, Rc<RefCell<Node>>>,

    top_nodes: Vec<Rc<RefCell<Node>>>,
}
