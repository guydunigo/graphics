//! Copy from steps : Trying to cull early

use glam::{Mat4, Vec3, Vec4Swizzles};
use std::{ops::DerefMut, time::Instant};
use winit::dpi::PhysicalSize;

use crate::{
    font::{self, TextWriter},
    maths::Vec4u,
    rasterizer::{
        Settings,
        cpu::{
            MINIMAL_AMBIANT_LIGHT, Rect, Triangle, bounding_box_triangle_2, cursor_buffer_index,
            format_debug, single_threaded::clean_resize_buffers, vec_cross_z,
        },
    },
    scene::{Camera, Node, Texture, World, to_cam_tr, to_raster},
    window::AppObserver,
};

use super::iterator::rasterize_triangle;

#[cfg(feature = "stats")]
use crate::rasterizer::cpu::Stats;

#[derive(Default)]
pub struct Steps2Engine {
    triangles: Vec<(Vec3, Vec3, Vec3)>,
    world_trs: Vec<Mat4>,
    to_cam_trs: Vec<Mat4>,
    textures: Vec<Texture>,

    t_raster: Vec<(Vec3, Vec3, Vec3)>,
    bounding_boxes: Vec<Rect>,
    p01p20: Vec<(Vec3, Vec3)>,
    light: Vec<f32>,
    depth_buffer: Vec<f32>,
}

impl Steps2Engine {
    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        ratio_w_h: f32,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        self.triangles.clear();
        self.world_trs.clear();
        self.to_cam_trs.clear();
        self.textures.clear();
        world.scene.top_nodes().iter().for_each(|n| {
            populate_nodes(
                settings,
                &world.camera,
                size,
                ratio_w_h,
                &mut self.triangles,
                &mut self.world_trs,
                &mut self.to_cam_trs,
                &mut self.textures,
                &n.borrow(),
            )
        });

        #[cfg(feature = "stats")]
        {
            stats.nb_triangles_tot = self.triangles.len();
        }

        self.t_raster.clear();
        self.t_raster.reserve(self.triangles.len());
        self.t_raster
            .extend(
                self.triangles
                    .iter()
                    .zip(self.to_cam_trs.iter())
                    .map(|((p0, p1, p2), tr)| {
                        (
                            to_raster(*p0, &world.camera, tr, size, ratio_w_h),
                            to_raster(*p1, &world.camera, tr, size, ratio_w_h),
                            to_raster(*p2, &world.camera, tr, size, ratio_w_h),
                        )
                    }),
            );
        // No need for self.to_cam_trs anymore.

        self.bounding_boxes.clear();
        self.bounding_boxes.reserve(self.triangles.len());
        while self.bounding_boxes.len() < self.triangles.len() {
            let i = self.bounding_boxes.len();
            // TODO: max_z >= MAX_DEPTH ?
            let bb = bounding_box_triangle_2(&self.t_raster[i], size);
            if !settings.culling_triangles
                || !(bb.min_x == bb.max_x
                    || bb.min_y == bb.max_y
                    || bb.max_z <= world.camera.z_near)
            {
                self.bounding_boxes.push(bb);
            } else {
                self.triangles.swap_remove(i);
                self.world_trs.swap_remove(i);
                self.textures.swap_remove(i);
                self.t_raster.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats.nb_triangles_sight = self.triangles.len();
        }

        ////////////////////////////////
        // Back face culling
        // If triangle normal and camera sight are in same direction (cross product > 0),
        // it's invisible.
        self.p01p20.clear();
        self.p01p20.reserve(self.triangles.len());
        while self.p01p20.len() < self.triangles.len() {
            let i = self.p01p20.len();
            let (p0, p1, p2) = &self.t_raster[i];
            let (p01, p20) = (p1 - p0, p0 - p2);
            if vec_cross_z(p01, p20) >= 0. {
                self.p01p20.push((p01, p20));
            } else {
                self.triangles.swap_remove(i);
                self.world_trs.swap_remove(i);
                self.textures.swap_remove(i);
                self.t_raster.swap_remove(i);
                self.bounding_boxes.swap_remove(i);
            }
        }

        #[cfg(feature = "stats")]
        {
            stats.nb_triangles_facing = self.triangles.len();
        }

        self.triangles
            .iter_mut()
            .zip(self.world_trs.iter())
            .for_each(|((p0, p1, p2), tr)| {
                *p0 = (tr * p0.extend(1.)).xyz();
                *p1 = (tr * p1.extend(1.)).xyz();
                *p2 = (tr * p2.extend(1.)).xyz();
            });

        ////////////////////////////////
        // Sunlight
        // Dot product gives negative if two vectors are opposed, so we compare light
        // vector to face normal vector to see if they are opposed (face is lit).
        //
        // Also simplifying colours.
        self.light.clear();
        self.light.reserve(self.triangles.len());
        self.light
            .extend(self.textures.iter_mut().zip(self.triangles.iter()).map(
                |(texture, (p0, p1, p2))| {
                    let triangle_normal = (p1 - p0).cross(p0 - p2).normalize();
                    let light = world
                        .sun_direction
                        .dot(triangle_normal)
                        .clamp(MINIMAL_AMBIANT_LIGHT, 1.);

                    // TODO: remove this test, just load correctly ?
                    // If a `Texture::VertexColor` has the same color for all triangles, then we can
                    // consider it like a `Texture::Color`.
                    if let Texture::VertexColor(c0, c1, c2) = texture
                        && c0 == c1
                        && c1 == c2
                    {
                        *texture = Texture::Color(*c0);
                    }

                    if let Texture::Color(col) = texture {
                        *texture =
                            Texture::Color((Vec4u::from_color_u32(*col) * light).as_color_u32());
                    }

                    light
                },
            ));

        self.t_raster
            .drain(..)
            .zip(self.textures.drain(..))
            .zip(self.bounding_boxes.drain(..))
            .zip(self.p01p20.drain(..))
            .zip(self.light.drain(..))
            .for_each(|(((((p0, p1, p2), material), bb), (p01, p20)), light)| {
                rasterize_triangle(
                    settings,
                    &Triangle {
                        p0,
                        p1,
                        p2,
                        material,
                    },
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
    }

    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        settings: &Settings,
        text_writer: &TextWriter,
        world: &World,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        app.last_buffer_fill_micros = clean_resize_buffers(&mut self.depth_buffer, buffer, size);

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

fn populate_nodes(
    settings: &Settings,
    camera: &Camera,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
    triangles: &mut Vec<(Vec3, Vec3, Vec3)>,
    world_trs: &mut Vec<Mat4>,
    to_cam_trs: &mut Vec<Mat4>,
    textures: &mut Vec<Texture>,
    node: &Node,
) {
    {
        let to_cam_tr = to_cam_tr(camera, &node.world_transform);
        if let Some(mesh) = node.mesh.as_ref()
            && (!settings.culling_meshes
                || mesh
                    .bounds
                    .is_visible_cpu(camera, &to_cam_tr, size, ratio_w_h))
        {
            let vert_count = mesh.surfaces.iter().map(|s| s.count).sum::<usize>() / 3;
            triangles.reserve(vert_count);
            world_trs.reserve(vert_count);
            to_cam_trs.reserve(vert_count);
            textures.reserve(vert_count);
            mesh.surfaces
                .iter()
                .filter(|s| {
                    !settings.culling_surfaces
                        || s.bounds.is_visible_cpu(camera, &to_cam_tr, size, ratio_w_h)
                })
                .for_each(|s| {
                    mesh.indices[s.start_index..s.start_index + s.count]
                        .chunks_exact(3)
                        .for_each(|is| {
                            triangles.push((
                                mesh.vertices[is[0]].position,
                                mesh.vertices[is[1]].position,
                                mesh.vertices[is[2]].position,
                            ));
                            world_trs.push(node.world_transform);
                            to_cam_trs.push(to_cam_tr);
                            textures.push(s.material);
                        });
                });
        }
    }

    node.children.iter().for_each(|c| {
        populate_nodes(
            settings,
            camera,
            size,
            ratio_w_h,
            triangles,
            world_trs,
            to_cam_trs,
            textures,
            &c.borrow(),
        )
    });
}
