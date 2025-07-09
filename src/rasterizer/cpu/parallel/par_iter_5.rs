//! Par_drain in a for_each loop
use glam::Vec3;
use rayon::prelude::*;
use std::sync::{Arc, atomic::AtomicU64};
use winit::dpi::PhysicalSize;

use crate::{
    maths::Vec4u,
    rasterizer::{
        cpu::{MINIMAL_AMBIANT_LIGHT, Triangle, bounding_box_triangle, world_to_raster_triangle},
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
pub struct ParIterEngine5 {
    triangles: Vec<Triangle>,
    depth_color_buffer: Arc<[AtomicU64]>,
}

impl ParIterEngine for ParIterEngine5 {
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
        self.triangles.par_drain(..).for_each(|t| {
            #[cfg(feature = "stats")]
            stats.nb_triangles_tot.fetch_add(1, Ordering::Relaxed);

            // TODO: explode ?
            let mut t_raster = world_to_raster_triangle(&t, camera, size, ratio_w_h);
            let bb = bounding_box_triangle(&t_raster, size);

            // TODO: max_z >= MAX_DEPTH ?
            if bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= camera.z_near {
                return;
            }

            #[cfg(feature = "stats")]
            stats.nb_triangles_sight.fetch_add(1, Ordering::Relaxed);

            let p01 = t_raster.p1 - t_raster.p0;
            let p20 = t_raster.p0 - t_raster.p2;

            ////////////////////////////////
            // Back face culling
            // If triangle normal and camera sight are in same direction (dot product > 0),
            // it's invisible.
            // Calculate only of normal z
            if p01.cross(p20).z <= 0. {
                return;
            }

            #[cfg(feature = "stats")]
            stats.nb_triangles_facing.fetch_add(1, Ordering::Relaxed);

            ////////////////////////////////
            // Sunlight
            // Dot product gives negative if two vectors are opposed, so we compare light
            // vector to face normal vector to see if they are opposed (face is lit).
            //
            // Also simplifying colours.
            let triangle_normal = (t.p1 - t.p0).cross(t.p0 - t.p2).normalize();
            let light = sun_direction
                .dot(triangle_normal)
                .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

            // If a `Texture::VertexColor` has the same color for all vertices, then we can
            // consider it like a `Texture::Color`.
            if let Texture::VertexColor(c0, c1, c2) = t_raster.material
                && c0 == c1
                && c1 == c2
            {
                t_raster.material = Texture::Color(c0);
            }

            if let Texture::Color(col) = t_raster.material {
                t_raster.material =
                    Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32());
            }

            rasterize_triangle(
                &t_raster,
                &self.depth_color_buffer,
                camera.z_near,
                size,
                settings,
                #[cfg(feature = "stats")]
                stats,
                &bb,
                light,
                p01,
                p20,
            );
        });
    }
}
