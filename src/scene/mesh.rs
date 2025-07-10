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
use winit::dpi::PhysicalSize;

use crate::scene::{Camera, local_to_clipspace};

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
        let bounds = Bounds::new(&vertices, &indices, 0, indices.len());
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
    pub fn new(
        vertices: &[Vertex],
        indices: &[usize],
        start_index: usize,
        count: usize,
        material: T,
    ) -> Self {
        GeoSurface {
            start_index,
            count,
            material,
            bounds: Bounds::new(vertices, indices, start_index, count),
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
    pub fn new(vertices: &[Vertex], indices: &[usize], start: usize, count: usize) -> Self {
        let default = vertices[indices[start]].position;
        let (min, max) = indices[start..start + count]
            .iter()
            .skip(1)
            .map(|i| vertices[*i])
            .fold((default, default), |(min, max), p| {
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
    /// From vulkan guide
    pub fn is_visible(&self, view_proj: &Mat4, transform: &Mat4) -> bool {
        let mut corners = [
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

        corners.iter_mut().for_each(|c| {
            let v = matrix * (self.origin + *c * self.extents).extend(1.);
            *c = v.xyz() / v.w;
        });

        let min = corners
            .iter()
            .copied()
            .fold(vec3(1.5, 1.5, 1.5), |a, b| a.min(b));
        let max = corners
            .iter()
            .copied()
            .fold(vec3(-1.5, -1.5, -1.5), |a, b| a.min(b));

        // Clip space box in view
        min.z <= 1. && max.z >= 0. && min.x <= 1. && max.x >= -1. && min.y <= 1. && max.y >= -1.
    }

    pub fn clip_space_origin_depth(&self, view_proj: &Mat4, transform: &Mat4) -> f32 {
        let projected_origin = view_proj * transform * self.origin.extend(1.);
        projected_origin.z
    }

    /// Done on my own
    pub fn is_visible_cpu(
        &self,
        camera: &Camera,
        to_cam_tr: &Mat4,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
    ) -> bool {
        let mut corners = [
            vec3(1., 1., 1.),
            vec3(1., 1., -1.),
            vec3(1., -1., 1.),
            vec3(1., -1., -1.),
            vec3(-1., 1., 1.),
            vec3(-1., 1., -1.),
            vec3(-1., -1., 1.),
            vec3(-1., -1., -1.),
        ];

        corners.iter_mut().for_each(|c| {
            *c = local_to_clipspace(
                camera,
                to_cam_tr,
                size,
                ratio_w_h,
                &(self.origin + *c * self.extents),
            );
        });

        let min = corners
            .iter()
            .copied()
            .reduce(|a, b| a.min(b))
            .unwrap()
            .clamp(Vec3::splat(-1.), Vec3::splat(1.));

        let max = corners
            .iter()
            .copied()
            .reduce(|a, b| a.max(b))
            .unwrap()
            .clamp(Vec3::splat(-1.), Vec3::splat(1.));

        // TODO: max_z >= MAX_DEPTH ?
        let res = !(min.x == max.x || min.y == max.y || max.z <= camera.z_near);

        assert_eq!(
            res,
            self.is_visible(
                &camera.view_mat(),
                &(to_cam_tr * camera.view_mat().inverse())
            ),
            "Not same visibility !"
        );

        res
    }
}

pub struct Node {
    /// If there is no parent or it was destroyed, weak won't upgrade.
    pub parent: Weak<RefCell<Node>>,
    pub children: Vec<Rc<RefCell<Node>>>,

    pub local_transform: Mat4,
    /// Cache :
    pub world_transform: Mat4,

    /// Actual mesh if any at this node
    pub mesh: Option<Rc<MeshAsset>>,
}

impl Node {
    pub fn parent_of(mut children: Vec<Rc<RefCell<Node>>>) -> Rc<RefCell<Self>> {
        Rc::new_cyclic(|f| {
            children
                .iter_mut()
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

    pub fn refresh_transform(&mut self, parent_mat: &Mat4) {
        self.world_transform = parent_mat * self.local_transform;
        self.children
            .iter()
            .for_each(|c| c.borrow_mut().refresh_transform(&self.world_transform));
    }

    pub fn transform(&mut self, tr: &Mat4) {
        // We split to rotate in place.
        let (tr_scale, tr_rot, tr_pos) = tr.to_scale_rotation_translation();
        let (scale, rot, pos) = self.local_transform.to_scale_rotation_translation();

        self.local_transform =
            Mat4::from_scale_rotation_translation(scale * tr_scale, tr_rot * rot, pos + tr_pos);
        self.refresh_transform(&Mat4::IDENTITY);
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
