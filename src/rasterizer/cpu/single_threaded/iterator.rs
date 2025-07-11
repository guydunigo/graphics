//! From original, but splitting the rasterize_triangle function tu have cleaner
//! iterator steps.

use std::ops::DerefMut;
use winit::dpi::PhysicalSize;

use crate::{
    maths::Vec4u,
    rasterizer::{
        Settings,
        cpu::{MINIMAL_AMBIANT_LIGHT, vec_cross_z, world_to_raster_triangle},
    },
    scene::{BoundingBox, Texture, Triangle, World},
};

use super::{SingleThreadedEngine, rasterize_triangle};

#[cfg(feature = "stats")]
use crate::rasterizer::cpu::Stats;

#[derive(Default)]
pub struct IteratorEngine {
    triangles: Vec<Triangle>,
    depth_buffer: Vec<f32>,
}

impl SingleThreadedEngine for IteratorEngine {
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

        self.triangles
            .drain(..)
            .inspect(|_| {
                #[cfg(feature = "stats")]
                {
                    nb_triangles_tot += 1;
                }
            })
            .map(|t| {
                let t_raster = world_to_raster_triangle(&t, &world.camera, size, ratio_w_h);
                (t, t_raster, BoundingBox::new(&t_raster, size))
            })
            .filter(|(_, _, bb)| !settings.culling_triangles || bb.is_visible(world.camera.z_near))
            .inspect(|_| {
                #[cfg(feature = "stats")]
                {
                    nb_triangles_sight += 1;
                }
            })
            .map(|(t, t_raster, bb)| {
                let p01 = t_raster.p1 - t_raster.p0;
                let p20 = t_raster.p0 - t_raster.p2;
                (t, t_raster, bb, p01, p20)
            })
            ////////////////////////////////
            // Back face culling
            // If triangle normal and camera sight are in same direction (cross product > 0),
            // it's invisible.
            .filter(|(_, _, _, p01, p20)| vec_cross_z(*p01, *p20) >= 0.)
            .inspect(|_| {
                #[cfg(feature = "stats")]
                {
                    nb_triangles_facing += 1;
                }
            })
            ////////////////////////////////
            // Sunlight
            // Dot product gives negative if two vectors are opposed, so we compare light
            // vector to face normal vector to see if they are opposed (face is lit).
            //
            // Also simplifying colours.
            .map(|(t, mut t_raster, bb, p01, p20)| {
                let triangle_normal = (t.p1 - t.p0).cross(t.p0 - t.p2).normalize();
                let light = world
                    .sun_direction
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

                match t_raster.material {
                    Texture::Color(col) => {
                        t_raster.material =
                            Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32());
                    }
                    Texture::VertexColor(ref mut c0, ref mut c1, ref mut c2) => {
                        *c0 = (Vec4u::from_color_u32(*c0) * light).as_color_u32();
                        *c1 = (Vec4u::from_color_u32(*c1) * light).as_color_u32();
                        *c2 = (Vec4u::from_color_u32(*c2) * light).as_color_u32();
                    }
                }

                (t_raster, bb, p01, p20)
            })
            .for_each(|(t_raster, bb, p01, p20)| {
                rasterize_triangle(
                    settings,
                    &t_raster,
                    buffer,
                    &mut self.depth_buffer[..],
                    world.camera.z_near,
                    size,
                    #[cfg(feature = "stats")]
                    stats,
                    &bb,
                    p01,
                    p20,
                )
            });

        #[cfg(feature = "stats")]
        {
            stats.nb_triangles_tot += nb_triangles_tot;
            stats.nb_triangles_sight += nb_triangles_sight;
            stats.nb_triangles_facing += nb_triangles_facing;
        }
    }
}
