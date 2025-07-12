use std::sync::{Arc, RwLock, Weak};

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles, vec3, vec4};
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    pub uv_x: f32,
    pub normal: Vec3,
    pub uv_y: f32,
    pub color: Vec4,
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: Default::default(),
            uv_x: Default::default(),
            normal: vec3(1., 0., 0.),
            uv_y: Default::default(),
            color: vec4(1., 1., 1., 1.),
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
    pub fn from_vertices(vertices: &[Vertex]) -> Self {
        let (min, max) = vertices.iter().fold(
            (vertices[0].position, vertices[0].position),
            |(min, max), p| (min.min(p.position), max.max(p.position)),
        );

        let extents = (max - min) / 2.;
        Self {
            origin: (max + min) / 2.,
            extents,
            // sphere_radius: extents.length(),
        }
    }

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

        BoundingBox {
            min_x: min.x,
            min_y: min.y,
            max_x: max.x,
            max_y: max.y,
            max_z: max.z,
        }
        .is_visible(camera.z_near)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox<T> {
    pub min_x: T,
    pub min_y: T,
    pub max_x: T,
    pub max_y: T,
    pub max_z: f32,
}

impl BoundingBox<u32> {
    pub fn new(t: &Triangle, size: PhysicalSize<u32>) -> Self {
        Self::new_2((t.p0, t.p1, t.p2), size)
    }

    pub fn new_2((p0, p1, p2): (Vec3, Vec3, Vec3), size: PhysicalSize<u32>) -> Self {
        // TODO: MAX_DEPTH
        let max_vec = vec3(size.width as f32 - 1., size.height as f32 - 1., 1000.);
        let min = p0.min(p1).min(p2).clamp(Vec3::ZERO, max_vec);
        let max = p0.max(p1).max(p2).clamp(Vec3::ZERO, max_vec);
        BoundingBox {
            min_x: min.x as u32,
            min_y: min.y as u32,
            max_x: max.x as u32,
            max_y: max.y as u32,
            max_z: max.z,
        }
    }
}

impl<T: PartialEq> BoundingBox<T> {
    /// Is visible in box has a width or height non null and is in front of camera :
    pub fn is_visible(&self, z_near: f32) -> bool {
        // TODO: max_z >= MAX_DEPTH ?
        !(self.min_x == self.max_x || self.min_y == self.max_y || self.max_z <= z_near)
    }
}

#[derive(Clone, Copy)]
pub struct Triangle {
    pub p0: Vec3,
    pub p1: Vec3,
    pub p2: Vec3,
    pub material: Texture,
}

/*
impl Triangle {
    pub fn min_z(&self) -> f32 {
        f32::min(self.p0.z, f32::min(self.p1.z, self.p2.z))
    }
}
*/

pub struct Node {
    /// If there is no parent or it was destroyed, weak won't upgrade.
    pub parent: Weak<RwLock<Node>>,
    pub children: Vec<Arc<RwLock<Node>>>,

    pub local_transform: Mat4,
    /// Cache :
    pub world_transform: Mat4,

    /// Actual mesh if any at this node
    pub mesh: Option<Arc<MeshAsset>>,
}

impl Node {
    pub fn new(local_transform: Mat4) -> Self {
        Self {
            parent: Default::default(),
            children: Default::default(),

            local_transform,
            world_transform: Default::default(),

            mesh: None,
        }
    }

    pub fn new_mesh(mesh: Arc<MeshAsset>, local_transform: Mat4) -> Self {
        Self {
            mesh: Some(mesh),
            ..Self::new(local_transform)
        }
    }

    pub fn parent_of(mut children: Vec<Arc<RwLock<Node>>>) -> Arc<RwLock<Self>> {
        Arc::new_cyclic(|f| {
            children
                .iter_mut()
                .for_each(|c| c.write().unwrap().parent = f.clone());
            let node = Node {
                parent: Default::default(),
                children,

                local_transform: Default::default(),
                world_transform: Default::default(),

                mesh: None,
            };
            RwLock::new(node)
        })
    }

    pub fn refresh_transform(&mut self, parent_mat: &Mat4) {
        self.world_transform = parent_mat * self.local_transform;
        self.children
            .iter()
            .for_each(|c| c.write().unwrap().refresh_transform(&self.world_transform));
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

            mesh: Some(Arc::new(value)),
        }
    }
}
