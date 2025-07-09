//! Grouping tasks on all, step by steps
//!
//! Quite memry hungry
//! TODO: we could re-allocate some everytime.
//! TODO: don't reserve too much before filters ?

use glam::Vec3;
use std::ops::DerefMut;
use winit::dpi::PhysicalSize;

use crate::{
    maths::Vec4u,
    rasterizer::{
        Settings,
        cpu::{
            MINIMAL_AMBIANT_LIGHT, Rect, Triangle, bounding_box_triangle, vec_cross_z,
            world_to_raster_triangle,
        },
    },
    scene::{Texture, World},
};

use super::{SingleThreadedEngine, iterator::rasterize_triangle};

#[cfg(feature = "stats")]
use crate::rasterizer::Stats;

#[derive(Default)]
pub struct StepsEngine {
    triangles: Vec<Triangle>,
    t_raster: Vec<Triangle>,
    bounding_boxes: Vec<Rect>,
    p01p20: Vec<(Vec3, Vec3)>,
    light: Vec<f32>,
    depth_buffer: Vec<f32>,
}

impl SingleThreadedEngine for StepsEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32> {
        &mut self.depth_buffer
    }

    fn triangles_mut(&mut self) -> &mut Vec<Triangle> {
        &mut self.triangles
    }

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        #[cfg(feature = "stats")]
        let mut nb_triangles_sight = 0;
        #[cfg(feature = "stats")]
        let mut nb_triangles_tot = 0;
        #[cfg(feature = "stats")]
        let mut nb_triangles_facing = 0;
        #[cfg(feature = "stats")]
        let mut nb_triangles_drawn = 0;
        #[cfg(feature = "stats")]
        let mut nb_pixels_tested = 0;
        #[cfg(feature = "stats")]
        let mut nb_pixels_in = 0;
        #[cfg(feature = "stats")]
        let mut nb_pixels_front = 0;
        #[cfg(feature = "stats")]
        let mut nb_pixels_written = 0;

        #[cfg(feature = "stats")]
        {
            nb_triangles_tot = self.triangles.len();
        }

        self.t_raster.clear();
        self.t_raster.reserve(self.triangles.len());
        // TODO: explode ?
        self.t_raster.extend(
            self.triangles
                .iter()
                .map(|t| world_to_raster_triangle(&t, &world.camera, size, ratio_w_h)),
        );

        self.bounding_boxes.clear();
        self.bounding_boxes.reserve(self.triangles.len());
        while self.bounding_boxes.len() < self.triangles.len() {
            let i = self.bounding_boxes.len();
            // TODO: max_z >= MAX_DEPTH ?
            let bb = bounding_box_triangle(&self.t_raster[i], size);
            if !(bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= world.camera.z_near) {
                self.bounding_boxes.push(bb);
            } else {
                self.triangles.swap_remove(i);
                self.t_raster.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            nb_triangles_sight = self.triangles.len();
        }

        ////////////////////////////////
        // Back face culling
        // If triangle normal and camera sight are in same direction (cross product > 0),
        // it's invisible.
        self.p01p20.clear();
        self.p01p20.reserve(self.triangles.len());
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
            nb_triangles_facing = self.triangles.len();
        }

        ////////////////////////////////
        // Sunlight
        // Dot product gives negative if two vectors are opposed, so we compare light
        // vector to face normal vector to see if they are opposed (face is lit).
        //
        // Also simplifying colours.
        self.light.clear();
        self.light.reserve(self.triangles.len());
        self.t_raster
            .iter_mut()
            .zip(self.triangles.iter())
            .for_each(|(t_raster, t)| {
                let triangle_normal = (t.p1 - t.p0).cross(t.p0 - t.p2).normalize();
                let light = world
                    .sun_direction
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

                if let Texture::Color(col) = t.material {
                    t_raster.material =
                        Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32());
                }

                self.light.push(light);
            });

        self.t_raster
            .drain(..)
            .zip(self.bounding_boxes.drain(..))
            .zip(self.p01p20.drain(..))
            .zip(self.light.drain(..))
            .for_each(|(((mut t_raster, bb), (p01, p20)), light)| {
                rasterize_triangle(
                    settings,
                    &mut t_raster,
                    buffer,
                    &mut self.depth_buffer[..],
                    world.camera.z_near,
                    size,
                    #[cfg(feature = "stats")]
                    stats,
                    &bb,
                    light,
                    p01,
                    p20,
                )
            });

        #[cfg(feature = "stats")]
        {
            stats.nb_triangles_tot += nb_triangles_tot;
            stats.nb_triangles_sight += nb_triangles_sight;
            stats.nb_triangles_facing += nb_triangles_facing;
            stats.nb_triangles_drawn += nb_triangles_drawn;
            stats.nb_pixels_tested += nb_pixels_tested;
            stats.nb_pixels_in += nb_pixels_in;
            stats.nb_pixels_front += nb_pixels_front;
            stats.nb_pixels_written += nb_pixels_written;
        }
    }
}
