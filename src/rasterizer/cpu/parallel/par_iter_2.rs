//! Like steps but parallel : par_bridge
use glam::Vec3;
use rayon::prelude::*;
use std::sync::{Arc, atomic::AtomicU64};
use winit::dpi::PhysicalSize;

use crate::{
    maths::Vec4u,
    rasterizer::{
        cpu::{
            MINIMAL_AMBIANT_LIGHT, Rect, Triangle, bounding_box_triangle, vec_cross_z,
            world_to_raster_triangle,
        },
        settings::Settings,
    },
    scene::{Camera, Texture},
};

use super::{ParIterEngine, rasterize_triangle};

#[cfg(feature = "stats")]
use super::ParStats;
#[cfg(feature = "stats")]
use std::sync::atomic::Ordering;

/// par_bridge
#[derive(Default, Clone)]
pub struct ParIterEngine2 {
    triangles: Vec<Triangle>,
    t_raster: Vec<Triangle>,
    bounding_boxes: Vec<Rect>,
    p01p20: Vec<(Vec3, Vec3)>,
    depth_color_buffer: Arc<[AtomicU64]>,
}

impl ParIterEngine for ParIterEngine2 {
    fn depth_color_buffer(&self) -> &Arc<[AtomicU64]> {
        &self.depth_color_buffer
    }

    fn depth_color_buffer_mut(&mut self) -> &mut Arc<[AtomicU64]> {
        &mut self.depth_color_buffer
    }

    fn triangles_mut(&mut self) -> &mut Vec<Triangle> {
        &mut self.triangles
    }

    fn rasterize_world(
        &mut self,
        settings: &Settings,
        camera: &Camera,
        sun_direction: Vec3,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &ParStats,
    ) {
        #[cfg(feature = "stats")]
        {
            stats
                .nb_triangles_tot
                .store(self.triangles.len(), Ordering::Relaxed);
        }

        // self.t_raster.clear();
        // self.t_raster.reserve(self.triangles.len());
        self.t_raster.par_extend(
            self.triangles
                .par_iter()
                .map(|t| world_to_raster_triangle(t, camera, size, ratio_w_h)),
        );

        // self.bounding_boxes.clear();
        // self.bounding_boxes.reserve(self.triangles.len());
        while self.bounding_boxes.len() < self.triangles.len() {
            let i = self.bounding_boxes.len();
            // TODO: max_z >= MAX_DEPTH ?
            let bb = bounding_box_triangle(&self.t_raster[i], size);
            if !(bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= camera.z_near) {
                self.bounding_boxes.push(bb);
            } else {
                self.triangles.swap_remove(i);
                self.t_raster.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats
                .nb_triangles_sight
                .store(self.triangles.len(), Ordering::Relaxed);
        }

        ////////////////////////////////
        // Back face culling
        // If triangle normal and camera sight are in same direction (cross product > 0),
        // it's invisible.
        // self.p01p20.clear();
        // self.p01p20.reserve(self.triangles.len());
        while self.p01p20.len() < self.triangles.len() {
            let i = self.p01p20.len();
            let t = &self.t_raster[i];
            let (p01, p20) = (t.p1 - t.p0, t.p0 - t.p2);
            if vec_cross_z(p01, p20) >= 0. {
                self.p01p20.push((p01, p20));
            } else {
                self.triangles.swap_remove(i);
                self.t_raster.swap_remove(i);
                self.bounding_boxes.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats
                .nb_triangles_facing
                .store(self.triangles.len(), Ordering::Relaxed);
        }

        ////////////////////////////////
        // Sunlight
        // Dot product gives negative if two vectors are opposed, so we compare light
        // vector to face normal vector to see if they are opposed (face is lit).
        //
        // Also simplifying colours.
        self.t_raster
            .iter_mut()
            .zip(self.triangles.drain(..))
            .par_bridge()
            .for_each(|(t_raster, t)| {
                let triangle_normal = (t.p1 - t.p0).cross(t.p0 - t.p2).normalize();
                let light = sun_direction
                    .dot(triangle_normal)
                    .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

                // TODO: remove this test, just load correctly ?
                // If a `Texture::VertexColor` has the same color for all vertices, then we can
                // consider it like a `Texture::Color`.
                if let Texture::VertexColor(c0, c1, c2) = t_raster.material
                    && c0 == c1
                    && c1 == c2
                {
                    t_raster.material = Texture::Color(c0);
                }

                match &mut t_raster.material {
                    Texture::Color(col) => {
                        t_raster.material =
                            Texture::Color((Vec4u::from_color_u32(*col) * light).as_color_u32());
                    }
                    Texture::VertexColor(c0, c1, c2) => {
                        *c0 = (Vec4u::from_color_u32(*c0) * light).as_color_u32();
                        *c1 = (Vec4u::from_color_u32(*c1) * light).as_color_u32();
                        *c2 = (Vec4u::from_color_u32(*c2) * light).as_color_u32();
                    }
                }
            });

        self.t_raster
            .drain(..)
            .zip(self.bounding_boxes.drain(..))
            .zip(self.p01p20.drain(..))
            .par_bridge()
            .for_each(|((t_raster, bb), (p01, p20))| {
                rasterize_triangle(
                    &t_raster,
                    &self.depth_color_buffer[..],
                    camera.z_near,
                    size,
                    settings,
                    #[cfg(feature = "stats")]
                    stats,
                    &bb,
                    p01,
                    p20,
                )
            });
    }
}
