mod iterator;
mod original;
mod steps;
mod steps2;

use glam::{Vec3, vec3};
pub use iterator::IteratorEngine;
pub use original::OriginalEngine;
pub use steps::StepsEngine;
pub use steps2::{Steps2Engine, populate_nodes_split};

use std::{ops::DerefMut, time::Instant};
use winit::dpi::PhysicalSize;

use crate::{
    font::{self, TextWriter},
    maths::Vec4u,
    rasterizer::{
        Settings,
        cpu::{Rect, edge_function},
    },
    scene::{DEFAULT_BACKGROUND_COLOR, Texture, World},
    window::AppObserver,
};

use super::{Triangle, buffer_index, cursor_buffer_index, format_debug, populate_nodes};

#[cfg(feature = "stats")]
use crate::rasterizer::cpu::Stats;

/// Common base for engines not requiring buffer synchronization.
pub trait SingleThreadedEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32>;

    fn triangles_mut(&mut self) -> &mut Vec<Triangle>;

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &mut Stats,
    );

    fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        app.last_buffer_fill_micros = clean_resize_buffers(self.depth_buffer_mut(), buffer, size);

        {
            let triangles = self.triangles_mut();
            // triangles.clear();
            world
                .scene
                .top_nodes()
                .iter()
                .for_each(|n| populate_nodes(triangles, &n.borrow()));

            /*
            match settings.sort_triangles {
                TriangleSorting::BackToFront => {
                    triangles.sort_by_key(|t| -t.min_z() as u32);
                }
                TriangleSorting::FrontToBack => {
                    triangles.sort_by_key(|t| t.min_z() as u32);
                }
                _ => (),
            }
            */
        }

        let ratio_w_h = size.width as f32 / size.height as f32;

        let t = Instant::now();
        self.rasterize_world(
            settings,
            world,
            buffer,
            size,
            ratio_w_h,
            #[cfg(feature = "stats")]
            stats,
        );
        app.last_rendering_micros = t.elapsed().as_micros();

        {
            let cursor_color = cursor_buffer_index(app.cursor(), size).map(|index| buffer[index]);
            let display = format_debug(
                settings,
                world,
                app,
                size,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize(buffer, size, font::PX, &display[..]);
        }
    }
}

/// - Resize `depth_buffer` and fill it with inifite depth
/// - Fill `buffer` with `DEFAULT_BACKGROUND_COLOR`
///
/// | `buffer` should be already resized.
fn clean_resize_buffers<B: DerefMut<Target = [u32]>>(
    depth_buffer: &mut Vec<f32>,
    buffer: &mut B,
    size: PhysicalSize<u32>,
) -> u128 {
    let t = Instant::now();
    buffer.fill(DEFAULT_BACKGROUND_COLOR);

    depth_buffer.resize(size.width as usize * size.height as usize, f32::INFINITY);
    depth_buffer.fill(f32::INFINITY);

    t.elapsed().as_micros()
}

fn draw_vertice_basic<B: DerefMut<Target = [u32]>>(
    buffer: &mut B,
    size: PhysicalSize<u32>,
    v: Vec3,
    texture: &Texture,
) {
    if v.x >= 1.
        && v.x < (size.width as f32) - 1.
        && v.y >= 1.
        && v.y < (size.height as f32) - 1.
        && let Some(i) = buffer_index(v, size)
    {
        let color = match texture {
            Texture::Color(col) => *col,
            // TODO: Better color calculus
            Texture::VertexColor(c0, c1, c2) => ((Vec4u::from_color_u32(*c0)
                + Vec4u::from_color_u32(*c1)
                + Vec4u::from_color_u32(*c2))
                / 3.)
                .as_color_u32(),
        };

        buffer[i] = color;
        buffer[i - 1] = color;
        buffer[i + 1] = color;
        buffer[i - (size.width as usize)] = color;
        buffer[i + (size.width as usize)] = color;
    }
}

pub fn rasterize_triangle<B: DerefMut<Target = [u32]>>(
    settings: &Settings,
    tri_raster: &Triangle,
    buffer: &mut B,
    depth_buffer: &mut [f32],
    z_near: f32,
    size: PhysicalSize<u32>,
    #[cfg(feature = "stats")] stats: &mut Stats,
    bb: &Rect,
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
                    let col_0 = Vec4u::from_color_u32(c0) / tri_raster.p0.z;
                    let col_1 = Vec4u::from_color_u32(c1) / tri_raster.p1.z;
                    let col_2 = Vec4u::from_color_u32(c2) / tri_raster.p2.z;

                    ((col_2 + (col_0 - col_2) * a12 + (col_1 - col_2) * a20) * depth).as_color_u32()
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
