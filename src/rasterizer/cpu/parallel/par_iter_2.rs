//! Like ParIter4 but calculating world transform in parallel.
use glam::{Mat4, Vec3, Vec4Swizzles};
use rayon::prelude::*;
use std::{
    ops::DerefMut,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use winit::dpi::PhysicalSize;

use crate::{
    font::{self, TextWriter},
    maths::Vec4u,
    rasterizer::{
        cpu::{
            MINIMAL_AMBIANT_LIGHT, Triangle, bounding_box_triangle, cursor_buffer_index,
            format_debug,
            parallel::{clean_resize_buffer, u64_to_color},
            world_to_raster_triangle,
        },
        settings::Settings,
    },
    scene::{Camera, Node, Texture},
    window::AppObserver,
};

use super::{ParIterEngine, rasterize_triangle};

#[cfg(feature = "stats")]
use super::ParStats;
#[cfg(feature = "stats")]
use std::sync::atomic::Ordering;

/// par_bridge
#[derive(Default, Clone)]
pub struct ParIterEngine2 {
    triangles: Vec<(Triangle, Mat4)>,
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
        unimplemented!()
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
        self.triangles
            .par_drain(..)
            .inspect(|_| {
                #[cfg(feature = "stats")]
                stats.nb_triangles_tot.fetch_add(1, Ordering::Relaxed);
            })
            .map(|(mut t, tr)| {
                t.p0 = (tr * t.p0.extend(1.)).xyz();
                t.p1 = (tr * t.p1.extend(1.)).xyz();
                t.p2 = (tr * t.p2.extend(1.)).xyz();
                t
            })
            .map(|t| {
                // TODO: explode ?
                let t_raster = world_to_raster_triangle(&t, camera, size, ratio_w_h);
                let bb = bounding_box_triangle(&t_raster, size);
                (t, t_raster, bb)
            })
            .filter(|(_, _, bb)| {
                // TODO: max_z >= MAX_DEPTH ?
                !(bb.min_x == bb.max_x || bb.min_y == bb.max_y || bb.max_z <= camera.z_near)
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

                (t_raster, bb, light, p01, p20)
            })
            .for_each(|(t_raster, bb, light, p01, p20)| {
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
                )
            });
    }

    fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &crate::scene::World,
        buffer: &mut B,
        mut size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        let original_size = size;

        // If x4 is set :
        let mut font_size = font::PX;
        size.width *= settings.oversampling as u32;
        size.height *= settings.oversampling as u32;
        if settings.parallel_text {
            font_size *= settings.oversampling as f32;
        }

        app.last_buffer_fill_micros = clean_resize_buffer(self.depth_color_buffer_mut(), size);

        {
            self.triangles.clear();
            world
                .scene
                .top_nodes()
                .iter()
                .for_each(|n| populate_nodes(&mut self.triangles, &n.borrow()));

            /*
            // TODO: Can't sort, not from camera view
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

        {
            let t = Instant::now();
            #[cfg(feature = "stats")]
            let par_stats = ParStats::from(&*stats);
            self.rasterize_world(
                settings,
                &world.camera,
                world.sun_direction,
                size,
                ratio_w_h,
                #[cfg(feature = "stats")]
                &par_stats,
            );
            #[cfg(feature = "stats")]
            par_stats.update_stats(stats);
            app.last_rendering_micros = t.elapsed().as_micros();
        }

        let depth_color_buffer = self.depth_color_buffer();

        if settings.parallel_text {
            let cursor_color = cursor_buffer_index(app.cursor(), size)
                .map(|index| u64_to_color(depth_color_buffer[index].load(Ordering::Relaxed)));
            let display = format_debug(
                settings,
                world,
                app,
                size,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize_par(depth_color_buffer, size, font_size, &display[..]);
        }

        // TODO: parallel (safe split ref vec)
        let t = Instant::now();
        if settings.oversampling > 1 {
            let oversampling_2 = settings.oversampling * settings.oversampling;

            (0..((original_size.height * original_size.width) as usize))
                .step_by(original_size.width as usize)
                .for_each(|j| {
                    let jx = j * oversampling_2;
                    (0..(original_size.width as usize)).for_each(|i| {
                        // println!(
                        //     "{jx4},{ix4} {} {} {} {}",
                        //     jx4 * 4 + ix4 * 2,
                        //     jx4 * 4 + ix4 * 2 + 1,
                        //     jx4 * 4 + size.width as usize + ix4 * 2,
                        //     jx4 * 4 + size.width as usize + ix4 * 2 + 1,
                        // );
                        let ix = i * settings.oversampling;

                        let color_avg: Vec4u = (0..(settings.oversampling * size.width as usize))
                            .step_by(size.width as usize)
                            .flat_map(|jo| {
                                (0..settings.oversampling).map(move |io| {
                                    Vec4u::from_color_u32(u64_to_color(
                                        depth_color_buffer[jx + jo + ix + io]
                                            .load(Ordering::Relaxed),
                                    ))
                                })
                            })
                            .sum();
                        let color_avg = color_avg / oversampling_2 as f32;
                        buffer[j + i] = color_avg.as_color_u32();
                    });
                });
        } else {
            (0..(size.width * size.height) as usize).for_each(|i| {
                buffer[i] = u64_to_color(depth_color_buffer[i].load(Ordering::Relaxed));
            });
        }
        app.last_buffer_copy_micros = t.elapsed().as_micros();

        if !settings.parallel_text {
            let cursor_color =
                cursor_buffer_index(app.cursor(), original_size).map(|index| buffer[index]);
            let display = format_debug(
                settings,
                world,
                app,
                original_size,
                cursor_color,
                #[cfg(feature = "stats")]
                stats,
            );
            text_writer.rasterize(buffer, original_size, font_size, &display[..]);
        }
    }
}

pub fn populate_nodes(triangles: &mut Vec<(Triangle, Mat4)>, node: &Node) {
    {
        // TODO: mesh + surface culling via bounding boxes ?
        if let Some(mesh) = node.mesh.as_ref() {
            triangles.reserve(mesh.surfaces.iter().map(|s| s.count / 3).sum());
            triangles.extend(mesh.surfaces.iter().flat_map(|s| {
                (0..s.count).step_by(3).map(|i| s.start_index + i).map(|i| {
                    (
                        Triangle {
                            p0: mesh.vertices[mesh.indices[i]].position,
                            p1: mesh.vertices[mesh.indices[i + 1]].position,
                            p2: mesh.vertices[mesh.indices[i + 2]].position,
                            material: s.material,
                        },
                        node.world_transform,
                    )
                })
            }));
        }
    }

    node.children
        .iter()
        .for_each(|c| populate_nodes(triangles, &c.borrow()));
}
