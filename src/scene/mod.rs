/// Describing the world
mod camera;
pub use camera::Camera;
mod mesh;
pub use mesh::Mesh;
pub mod obj_file;
mod triangle;
pub use triangle::{Texture, Triangle};
mod mesh_library;

use crate::maths::Vec3f;

pub const DEFAULT_BACKGROUND_COLOR: u32 = 0xff181818;

#[derive(Debug, Clone)]
pub struct World {
    pub meshes: Vec<Mesh>,
    pub camera: Camera,
    pub sun_direction: Vec3f,
}

impl Default for World {
    fn default() -> Self {
        World {
            meshes: vec![
                Mesh::from(Triangle::default()).with_translation_to(Vec3f::new(0., 0., -10.)),
                mesh_library::base_pyramid(),
                obj_file::import_triangles_and_diffuse(obj_file::SUZANNE_OBJ_PATH),
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
