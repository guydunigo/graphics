use crate::maths::{Rotation, Vec3f};

use super::Triangle;

/// Physical object made of triangle faces
#[derive(Debug, Clone)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
    pub pos: Vec3f,
    pub rot: Rotation,
    pub scale: f32,
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            triangles: Default::default(),
            pos: Default::default(),
            rot: Default::default(),
            scale: 1.,
        }
    }
}

impl From<Triangle> for Mesh {
    fn from(value: Triangle) -> Self {
        Mesh {
            triangles: vec![value],
            ..Default::default()
        }
    }
}

impl Mesh {
    pub fn with_translation_to(self, new_pos: Vec3f) -> Self {
        Self {
            pos: new_pos,
            ..self
        }
    }

    pub fn to_world_triangles(&self) -> impl Iterator<Item = Triangle> {
        self.triangles
            .iter()
            .map(|t| t.scale_rot_move(self.scale, &self.rot, self.pos))
    }
}
