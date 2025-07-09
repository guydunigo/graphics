//! From original, but splitting the rasterize_triangle function tu have cleaner
//! iterator steps.

use glam::{Vec3, vec3};
use std::ops::DerefMut;
use winit::dpi::PhysicalSize;

use crate::{
    maths::Vec4u,
    rasterizer::{
        Settings,
        cpu::{
            MINIMAL_AMBIANT_LIGHT, Rect, Triangle, bounding_box_triangle, edge_function,
            vec_cross_z, world_to_raster_triangle,
        },
    },
    scene::{Texture, World},
};

use super::{SingleThreadedEngine, draw_vertice_basic};

#[cfg(feature = "stats")]
use crate::rasterizer::Stats;

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

        self.triangles
            .drain(..)
            .inspect(|_| {
                #[cfg(feature = "stats")]
                {
                    nb_triangles_tot += 1;
                }
            })
            .map(|t| {
                // TODO: explode ?
                let t_raster = world_to_raster_triangle(&t, &world.camera, size, ratio_w_h);
                (t, t_raster, bounding_box_triangle(&t_raster, size))
            })
            .filter(|(_, _, bb)| {
                // TODO: max_z >= MAX_DEPTH ?
                !(bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= world.camera.z_near)
            })
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

                if let Texture::Color(col) = t_raster.material {
                    t_raster.material =
                        Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32());
                }

                (t_raster, bb, light, p01, p20)
            })
            .for_each(|(mut t_raster, bb, light, p01, p20)| {
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

fn rasterize_triangle<B: DerefMut<Target = [u32]>>(
    settings: &Settings,
    tri_raster: &mut Triangle,
    buffer: &mut B,
    depth_buffer: &mut [f32],
    z_near: f32,
    size: PhysicalSize<u32>,
    #[cfg(feature = "stats")] stats: &mut Stats,
    bb: &Rect,
    light: f32,
    p01: Vec3,
    p20: Vec3,
) {
    #[cfg(feature = "stats")]
    let mut was_drawn = false;

    let p12 = tri_raster.p2 - tri_raster.p1;

    let tri_area = edge_function(p20, p01);

    (bb.min_x..=bb.max_x)
        .flat_map(|x| (bb.min_y..=bb.max_y).map(move |y| vec3(x as f32, y as f32, 0.)))
        .for_each(|pixel| {
            // TODO: split to iterator ?

            #[cfg(feature = "stats")]
            {
                stats.nb_pixels_tested += 1;
            }

            let e01 = edge_function(p01, pixel - tri_raster.p0);
            let e12 = edge_function(p12, pixel - tri_raster.p1);
            let e20 = edge_function(p20, pixel - tri_raster.p2);

            // If negative for the 3 : we're outside the triangle.
            if e01 < 0. || e12 < 0. || e20 < 0. {
                return;
            }

            #[cfg(feature = "stats")]
            {
                stats.nb_pixels_in += 1;
            }

            let a12 = e12 / tri_area;
            let a20 = e20 / tri_area;

            // let depth_2 = 1.
            //     / (1. / tri_raster.p2.pos.z * (e01 / tri_area)
            //         + 1. / tri_raster.p0.pos.z * a12
            //         + 1. / tri_raster.p1.pos.z * a20);
            // Because a01 + a12 + a20 = 1., we can avoid a division and not compute a01.
            // The terms Z1-Z0 and Z2-Z0 can generally be precomputed, which simplifies the computation of Z to two additions and two multiplications. This optimization is worth mentioning because GPUs utilize it, and it's often discussed for essentially this reason.

            // Depth doesn't evolve linearly (its inverse does).
            let p2_z_inv = 1. / tri_raster.p2.z;
            let depth = 1.
                / (p2_z_inv
                    + (1. / tri_raster.p0.z - p2_z_inv) * a12
                    + (1. / tri_raster.p1.z - p2_z_inv) * a20);

            // Depth correction of other properties :
            // Divide each value by the point Z coord and finally multiply by depth.

            if depth <= z_near {
                return;
            }

            #[cfg(feature = "stats")]
            {
                stats.nb_pixels_front += 1;
            }

            let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

            if depth >= depth_buffer[index] {
                return;
            }

            #[cfg(feature = "stats")]
            {
                was_drawn = true;
                stats.nb_pixels_written += 1;
            }

            let col = match tri_raster.material {
                Texture::Color(col) => col,
                Texture::VertexColor(c0, c1, c2) => {
                    // TODO: Optimize color calculus
                    let col_2 = Vec4u::from_color_u32(c2) / tri_raster.p2.z;

                    ((col_2
                        + (Vec4u::from_color_u32(c0) / tri_raster.p0.z - col_2) * a12
                        + (Vec4u::from_color_u32(c1) / tri_raster.p1.z - col_2) * a20)
                        * (depth * light))
                        .as_color_u32()
                }
            };

            buffer[index] = col;
            depth_buffer[index] = depth;
        });

    #[cfg(feature = "stats")]
    if was_drawn {
        stats.nb_triangles_drawn += 1;
    }

    if settings.show_vertices {
        draw_vertice_basic(buffer, size, tri_raster.p0, &tri_raster.material);
        draw_vertice_basic(buffer, size, tri_raster.p1, &tri_raster.material);
        draw_vertice_basic(buffer, size, tri_raster.p2, &tri_raster.material);
    }
}
