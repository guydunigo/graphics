use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use rayon::prelude::*;
use winit::dpi::PhysicalSize;

use crate::{
    font::{self, TextWriter},
    maths::Vec4u,
    rasterizer::{
        MINIMAL_AMBIANT_LIGHT, bounding_box_triangle, cursor_buffer_index, format_debug,
        settings::Settings, world_to_raster_triangle,
    },
    scene::Texture,
    window::AppObserver,
};

use super::{clean_resize_buffer, rasterize_triangle, scene::World, u64_to_color};

#[cfg(feature = "stats")]
use super::ParStats;
#[cfg(feature = "stats")]
use crate::rasterizer::Stats;

/// Test parallel iter directly on memory array of triangles
///
/// This needs a modified world where triangles have pointers to meshes...
#[derive(Default, Debug, Clone)]
pub struct ParIterEngine2 {
    depth_color_buffer: Arc<[AtomicU64]>,
    world: Option<World>,
}

impl ParIterEngine2 {
    pub fn rasterize<B: std::ops::DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &crate::scene::World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        app.last_buffer_fill_micros = clean_resize_buffer(&mut self.depth_color_buffer, size);

        let par_world = self.world.get_or_insert_with(|| world.into());

        {
            let t = Instant::now();
            #[cfg(feature = "stats")]
            let par_stats = ParStats::from(&*stats);
            rasterize_world(
                settings,
                par_world,
                &self.depth_color_buffer,
                size,
                #[cfg(feature = "stats")]
                &par_stats,
            );
            #[cfg(feature = "stats")]
            par_stats.update_stats(stats);
            app.last_rendering_micros = Instant::now().duration_since(t).as_micros();
        }

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
            text_writer.rasterize_par(&self.depth_color_buffer, size, font::PX, &display[..]);
        }

        // TODO: parallel (safe split ref vec)
        let t = Instant::now();
        (0..(size.width * size.height) as usize).for_each(|i| {
            buffer[i] = u64_to_color(self.depth_color_buffer[i].load(Ordering::Relaxed));
        });
        app.last_buffer_copy_micros = Instant::now().duration_since(t).as_micros();
    }
}

pub fn rasterize_world(
    settings: &Settings,
    world: &World,
    depth_color_buffer: &[AtomicU64],
    size: PhysicalSize<u32>,
    #[cfg(feature = "stats")] stats: &ParStats,
) {
    let ratio_w_h = size.width as f32 / size.height as f32;

    world
        .triangles()
        .par_iter()
        .filter_map(|t| t.upgrade())
        .filter_map(|t| t.to_world())
        .inspect(|_| {
            #[cfg(feature = "stats")]
            stats.nb_triangles_tot.fetch_add(1, Ordering::Relaxed);
        })
        .map(|t| {
            // TODO: explode ?
            let t_raster = world_to_raster_triangle(&t, &world.camera, size, ratio_w_h);
            let bb = bounding_box_triangle(&t_raster, size);
            (t, t_raster, bb)
        })
        .filter(|(_, _, bb)| {
            // TODO: max_z >= MAX_DEPTH ?
            !(bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= world.camera.z_near)
        })
        .inspect(|_| {
            #[cfg(feature = "stats")]
            stats.nb_triangles_sight.fetch_add(1, Ordering::Relaxed);
        })
        .map(|(t, t_raster, bb)| {
            let p01 = t_raster.p1 - t_raster.p0;
            let p20 = t_raster.p0 - t_raster.p2;
            (t, t_raster, bb, p01, p20)
        })
        ////////////////////////////////
        // Back face culling
        // If triangle normal and camera sight are in same direction (dot product > 0),
        // it's invisible.
        .filter(|(_, _, _, p01, p20)| {
            // Calculate only of normal z
            let raster_normale = p01.cross(*p20);
            raster_normale.z >= 0.
        })
        .inspect(|_| {
            #[cfg(feature = "stats")]
            stats.nb_triangles_facing.fetch_add(1, Ordering::Relaxed);
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
            if let Texture::VertexColor(c0, c1, c2) = t_raster.texture
                && c0 == c1
                && c1 == c2
            {
                t_raster.texture = Texture::Color(c0);
            }

            if let Texture::Color(col) = t_raster.texture {
                t_raster.texture =
                    Texture::Color((Vec4u::from_color_u32(col) * light).as_color_u32());
            }

            (t_raster, bb, light, p01, p20)
        })
        .for_each(|(t_raster, bb, light, p01, p20)| {
            rasterize_triangle(
                &t_raster,
                depth_color_buffer,
                world.camera.z_near,
                size,
                settings,
                #[cfg(feature = "stats")]
                stats,
                &bb,
                light,
                p01,
                p20,
            )
        });
}
