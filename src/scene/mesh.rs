/*
use crate::maths::{Rotation, Vec3f};
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
*/

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use glam::{Mat4, Vec3, Vec4Swizzles, vec3};

use super::Texture;

pub struct MeshAsset {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<usize>,
    pub surfaces: Vec<GeoSurface<Texture>>,
    pub bounds: Bounds,
}

impl MeshAsset {
    pub fn new(
        vertices: Vec<Vertex>,
        indices: Vec<usize>,
        surfaces: Vec<GeoSurface<Texture>>,
    ) -> Self {
        let bounds = Bounds::new(&mut vertices.iter());
        Self {
            vertices,
            indices,
            surfaces,
            bounds,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    // pub uv_x: f32,
    // pub normal: Vec3,
    // pub uv_y: f32,
    // pub color: Vec4,
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: Default::default(),
            // uv_x: Default::default(),
            // normal: vec3(1., 0., 0.),
            // uv_y: Default::default(),
            // color: vec4(1., 1., 1., 1.),
        }
    }
}

pub struct GeoSurface<T> {
    pub start_index: usize,
    pub count: usize,
    pub material: T,
    pub bounds: Bounds,
}

impl<T> GeoSurface<T> {
    pub fn new(vertices: &[Vertex], start_index: usize, count: usize, material: T) -> Self {
        GeoSurface {
            start_index,
            count,
            material,
            bounds: Bounds::new(&mut vertices[start_index..start_index + count].iter()),
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Bounds {
    pub origin: Vec3,
    pub extents: Vec3,
    // pub sphere_radius: f32,
}

impl Bounds {
    pub fn new<'a, T: Iterator<Item = &'a Vertex>>(vertices: &mut T) -> Self {
        let mut vertices = vertices.peekable();
        let default = vertices.peek().unwrap().position;
        let (min, max) = vertices.fold((default, default), |(min, max), p| {
            (min.min(p.position), max.max(p.position))
        });

        let extents = (max - min) / 2.;
        Self {
            origin: (max + min) / 2.,
            extents,
            // sphere_radius: extents.length(),
        }
    }

    // TODO: is it optimal ?
    // TODO: glitchy for large objects in front and behind camera
    pub fn is_visible(&self, view_proj: &Mat4, transform: &Mat4) -> bool {
        let corners = [
            vec3(1., 1., 1.),
            vec3(1., 1., -1.),
            vec3(1., -1., 1.),
            vec3(1., -1., -1.),
            vec3(-1., 1., 1.),
            vec3(-1., 1., -1.),
            vec3(-1., -1., 1.),
            vec3(-1., -1., -1.),
        ];

        let matrix = view_proj * transform;

        let min = vec3(1.5, 1.5, 1.5);
        let max = vec3(-1.5, -1.5, -1.5);

        let (min, max) = corners.iter().fold((min, max), |(min, max), c| {
            let v = matrix * (self.origin + c * self.extents).extend(1.);
            let v = v.xyz() / v.w;
            (min.min(v), max.max(v))
        });

        // Clip space box in view
        min.z <= 1. && max.z >= 0. && min.x <= 1. && max.x >= -1. && min.y <= 1. && max.y >= -1.
    }

    pub fn clip_space_origin_depth(&self, view_proj: &Mat4, transform: &Mat4) -> f32 {
        let projected_origin = view_proj * transform * self.origin.extend(1.);
        projected_origin.z
    }
}

pub struct Node {
    /// If there is no parent or it was destroyed, weak won't upgrade.
    pub parent: Weak<Node>,
    pub children: Vec<Rc<Node>>,

    pub local_transform: Mat4,
    /// Cache :
    pub world_transform: RefCell<Mat4>,

    /// Actual mesh if any at this node
    pub mesh: Option<Rc<MeshAsset>>,
}

impl Node {
    pub fn parent_of(mut children: Vec<Rc<Node>>) -> Rc<Self> {
        Rc::new_cyclic(|f| {
            children
                .iter_mut()
                .for_each(|c| Rc::get_mut(c).unwrap().parent = f.clone());
            Node {
                parent: Default::default(),
                children,

                local_transform: Default::default(),
                world_transform: Default::default(),

                mesh: None,
            }
        })
    }

    pub fn refresh_transform(&self, parent_mat: &Mat4) {
        let world_transform = parent_mat * self.local_transform;
        self.children
            .iter()
            .for_each(|c| c.refresh_transform(&world_transform));
        *self.world_transform.borrow_mut() = world_transform;
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
