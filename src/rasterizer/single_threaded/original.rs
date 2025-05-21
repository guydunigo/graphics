//! First implementation, putting the whole triangle rasterizing in a single function.

use crate::{
    maths::{Vec3f, Vec4u},
    rasterizer::{
        MINIMAL_AMBIANT_LIGHT, Settings, bounding_box_triangle, edge_function,
        settings::TriangleSorting, world_to_raster_triangle,
    },
    scene::{Camera, Mesh, Texture, Triangle, World},
};
use std::ops::DerefMut;
use winit::dpi::PhysicalSize;

use super::{SingleThreadedEngine, draw_vertice_basic};

#[cfg(feature = "stats")]
use crate::rasterizer::Stats;

#[derive(Default, Debug, Clone)]
pub struct OriginalEngine {
    depth_buffer: Vec<f32>,
}

impl SingleThreadedEngine for OriginalEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32> {
        &mut self.depth_buffer
    }

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        depth_buffer: &mut [f32],
        size: PhysicalSize<u32>,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        let triangles = world.meshes.iter().flat_map(Mesh::to_world_triangles);

        let ratio_w_h = size.width as f32 / size.height as f32;

        let f = |f| {
            #[cfg(feature = "stats")]
            {
                stats.nb_triangles_tot += 1;
            }
            rasterize_triangle(
                settings,
                world,
                &f,
                buffer,
                depth_buffer,
                &world.camera,
                size,
                ratio_w_h,
                #[cfg(feature = "stats")]
                stats,
            );
        };

        match settings.sort_triangles {
            TriangleSorting::None => triangles.for_each(f),
            TriangleSorting::BackToFront => {
                let mut array: Vec<Triangle> = triangles.collect();
                array.sort_by_key(|t| -t.min_z() as u32);
                array.drain(..).for_each(f);
            }
            TriangleSorting::FrontToBack => {
                let mut array: Vec<Triangle> = triangles.collect();
                array.sort_by_key(|t| t.min_z() as u32);
                array.drain(..).for_each(f);
            }
        }
    }
}

fn rasterize_triangle<B: DerefMut<Target = [u32]>>(
    settings: &Settings,
    world: &World,
    triangle: &Triangle,
    buffer: &mut B,
    depth_buffer: &mut [f32],
    cam: &Camera,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
    #[cfg(feature = "stats")] stats: &mut Stats,
) {
    let tri_raster = world_to_raster_triangle(triangle, cam, size, ratio_w_h);

    let bb = bounding_box_triangle(&tri_raster, size);
    // TODO: max_z >= MAX_DEPTH ?
    if bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= cam.z_near {
        return;
    }

    #[cfg(feature = "stats")]
    {
        stats.nb_triangles_sight += 1;
    }

    ////////////////////////////////
    // Sunlight
    // Dot product gives negative if two vectors are opposed, so we compare light vector to
    // face normal vector to see if they are opposed (face is lit).

    // TODO: calculate before ?
    let triangle_normal = (triangle.p1 - triangle.p0)
        .cross(triangle.p0 - triangle.p2)
        .normalize();

    let light = world
        .sun_direction
        .dot(triangle_normal)
        .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

    ////////////////////////////////
    // Back face culling
    // If triangle normal and camera sight are in same direction (dot product > 0), it's invisible.

    let p01 = tri_raster.p1 - tri_raster.p0;
    let p20 = tri_raster.p0 - tri_raster.p2;

    let raster_normale = p01.cross(p20);
    // Calculate only of normal z
    if raster_normale.z < 0. {
        return;
    }

    #[cfg(feature = "stats")]
    {
        stats.nb_triangles_facing += 1;
    }

    ////////////////////////////////
    #[cfg(feature = "stats")]
    let mut was_drawn = false;

    let p12 = tri_raster.p2 - tri_raster.p1;

    let tri_area = edge_function(p20, p01);

    // TODO: Optimize color calculus
    let texture = match tri_raster.texture {
        Texture::Color(col) => Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32()),
        _ => tri_raster.texture,
    };

    (bb.min_x..=bb.max_x)
        .flat_map(|x| {
            (bb.min_y..=bb.max_y).map(move |y| Vec3f {
                x: x as f32,
                y: y as f32,
                z: 0.,
            })
        })
        .for_each(|pixel| {
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

            if depth <= cam.z_near {
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

            let col = match texture {
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
        draw_vertice_basic(buffer, size, tri_raster.p0, &tri_raster.texture);
        draw_vertice_basic(buffer, size, tri_raster.p1, &tri_raster.texture);
        draw_vertice_basic(buffer, size, tri_raster.p2, &tri_raster.texture);
    }
}
