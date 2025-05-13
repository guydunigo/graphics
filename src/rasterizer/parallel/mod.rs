mod par_iter_2;
mod par_iter_3;
mod par_iter_4;
mod par_iter_5;
mod scene;

use rayon::prelude::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

pub use par_iter_2::ParIterEngine2;
pub use par_iter_3::ParIterEngine3;
pub use par_iter_4::ParIterEngine4;
pub use par_iter_5::ParIterEngine5;
use winit::dpi::PhysicalSize;

use crate::{
    font::TextWriter,
    maths::{Vec3f, Vec4u},
    scene::{DEFAULT_BACKGROUND_COLOR, Texture, Triangle, World},
    window::AppObserver,
};

use super::{
    Rect, buffer_index, cursor_buffer_index, edge_function, format_debug, settings::Settings,
};

const DEPTH_PRECISION: f32 = 2048.;
const DEFAULT_DEPTH: u32 = u32::MAX;
const DEFAULT_DEPTH_COLOR: u64 = ((DEFAULT_DEPTH as u64) << 32) | (DEFAULT_BACKGROUND_COLOR as u64);

pub trait ParIterEngine {
    fn depth_color_buffer(&self) -> &Arc<[AtomicU64]>;
    fn depth_color_buffer_mut(&mut self) -> &mut Arc<[AtomicU64]>;

    fn rasterize_world(
        settings: &Settings,
        world: &World,
        depth_color_buffer: &[AtomicU64],
        size: PhysicalSize<u32>,
        #[cfg(feature = "stats")] stats: &Stats,
    );

    fn rasterize<B: std::ops::DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &crate::scene::World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        app.last_buffer_fill_micros = clean_resize_buffer(self.depth_color_buffer_mut(), size);

        let depth_color_buffer = self.depth_color_buffer();

        let t = Instant::now();
        Self::rasterize_world(
            settings,
            world,
            depth_color_buffer,
            size,
            #[cfg(feature = "stats")]
            &stats,
        );
        app.last_rendering_micros = Instant::now().duration_since(t).as_micros();

        {
            let cursor_color = cursor_buffer_index(&app.cursor(), size).map(|index| buffer[index]);
            let display = format_debug(
                settings,
                world,
                app,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize_par(depth_color_buffer, size, &display[..]);
        }

        let t = Instant::now();
        (0..(size.width * size.height) as usize).for_each(|i| {
            buffer[i] = u64_to_color(depth_color_buffer[i].load(Ordering::Relaxed));
        });
        app.last_buffer_copy_micros = Instant::now().duration_since(t).as_micros();
    }
}

fn rasterize_triangle(
    tri_raster: &Triangle,
    depth_color_buffer: &[AtomicU64],
    z_near: f32,
    size: PhysicalSize<u32>,
    settings: &Settings,
    #[cfg(feature = "stats")] stats: &Stats,
    bb: &Rect,
    light: f32,
    p01: Vec3f,
    p20: Vec3f,
) {
    #[cfg(feature = "stats")]
    let was_drawn = AtomicBool::new(false);

    let p12 = tri_raster.p2 - tri_raster.p1;

    let tri_area = edge_function(p20, p01);

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
            stats.nb_pixels_tested.fetch_add(1, Ordering::Relaxed);

            let e01 = edge_function(p01, pixel - tri_raster.p0);
            let e12 = edge_function(p12, pixel - tri_raster.p1);
            let e20 = edge_function(p20, pixel - tri_raster.p2);

            // If negative for the 3 : we're outside the triangle.
            if e01 < 0. || e12 < 0. || e20 < 0. {
                return;
            }

            #[cfg(feature = "stats")]
            stats.nb_pixels_in.fetch_add(1, Ordering::Relaxed);

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
            stats.nb_pixels_front.fetch_add(1, Ordering::Relaxed);

            let index = (pixel.x as usize) + (pixel.y as usize) * size.width as usize;

            let depth_u64 = depth_to_u64(depth);

            let col = match tri_raster.texture {
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
            } as u64
                | depth_u64;

            #[cfg(not(feature = "stats"))]
            depth_color_buffer[index].fetch_min(col, Ordering::Relaxed);
            #[cfg(feature = "stats")]
            let res = depth_color_buffer[index].fetch_min(col, Ordering::Relaxed) > depth_u64;
            #[cfg(feature = "stats")]
            was_drawn.store(res, Ordering::Relaxed);
            #[cfg(feature = "stats")]
            stats
                .nb_pixels_written
                .fetch_add(res as usize, Ordering::Relaxed);
        });

    #[cfg(feature = "stats")]
    if was_drawn.load(Ordering::Relaxed) {
        stats.nb_triangles_drawn.fetch_add(1, Ordering::Relaxed);
    }

    if settings.show_vertices {
        draw_vertice_basic(depth_color_buffer, size, tri_raster.p0, &tri_raster.texture);
        draw_vertice_basic(depth_color_buffer, size, tri_raster.p1, &tri_raster.texture);
        draw_vertice_basic(depth_color_buffer, size, tri_raster.p2, &tri_raster.texture);
    }
}

fn init_buffer<T, F: Fn() -> T>(tot_size: usize, f: F) -> Arc<[T]> {
    let mut v = Vec::with_capacity(tot_size);
    v.resize_with(tot_size, f);
    v.into()
}

/// Resize `depth_color_buffer` if necessary and fills it with [`Self::DEFAULT_DEPTH_COLOR`]
///
/// Returns time as microseconds.
fn clean_resize_buffer(depth_color_buffer: &mut Arc<[AtomicU64]>, size: PhysicalSize<u32>) -> u128 {
    let t = Instant::now();
    let tot_size = (size.width * size.height) as usize;

    if depth_color_buffer.len() >= tot_size {
        depth_color_buffer
            .par_iter()
            .take(tot_size)
            .for_each(|v| v.store(DEFAULT_DEPTH_COLOR, Ordering::Relaxed))
    } else {
        *depth_color_buffer = init_buffer(tot_size, || AtomicU64::new(DEFAULT_DEPTH_COLOR));
    }
    Instant::now().duration_since(t).as_micros()
}

const fn depth_to_u64(depth: f32) -> u64 {
    ((depth * DEPTH_PRECISION) as u64) << 32
}

const fn u64_to_color(depth_color: u64) -> u32 {
    (0xffffffff & depth_color) as u32
}

#[cfg(feature = "stats")]
#[derive(Default, Debug)]
pub struct Stats {
    pub nb_triangles_tot: AtomicUsize,
    pub nb_triangles_sight: AtomicUsize,
    pub nb_triangles_facing: AtomicUsize,
    pub nb_triangles_drawn: AtomicUsize,
    pub nb_pixels_tested: AtomicUsize,
    pub nb_pixels_in: AtomicUsize,
    pub nb_pixels_front: AtomicUsize,
    pub nb_pixels_written: AtomicUsize,
    // pub misc: String,
}

fn draw_vertice_basic(
    depth_color_buffer: &[AtomicU64],
    size: PhysicalSize<u32>,
    v: Vec3f,
    texture: &Texture,
) {
    if v.x >= 1. && v.x < (size.width as f32) - 1. && v.y >= 1. && v.y < (size.height as f32) - 1. {
        if let Some(i) = buffer_index(v, size) {
            let color = match texture {
                Texture::Color(col) => *col,
                // TODO: Better color calculus
                Texture::VertexColor(c0, c1, c2) => ((Vec4u::from_color_u32(*c0)
                    + Vec4u::from_color_u32(*c1)
                    + Vec4u::from_color_u32(*c2))
                    / 3.)
                    .as_color_u32(),
            } as u64;

            depth_color_buffer[i].store(color, Ordering::Relaxed);
            depth_color_buffer[i - 1].store(color, Ordering::Relaxed);
            depth_color_buffer[i + 1].store(color, Ordering::Relaxed);
            depth_color_buffer[i - (size.width as usize)].store(color, Ordering::Relaxed);
            depth_color_buffer[i + (size.width as usize)].store(color, Ordering::Relaxed);
        }
    }
}
