#![cfg(feature = "cpu")]

mod cpu_engine;
mod parallel;
mod single_threaded;

use glam::{Vec3, Vec4Swizzles};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::Settings;
use crate::{
    scene::{Camera, Node, Texture, World},
    window::AppObserver,
};
pub use cpu_engine::CPUEngine;

#[cfg(feature = "stats")]
#[derive(Default, Debug, Clone)]
pub struct Stats {
    pub nb_triangles_tot: usize,
    pub nb_triangles_sight: usize,
    pub nb_triangles_facing: usize,
    pub nb_triangles_drawn: usize,
    pub nb_pixels_tested: usize,
    pub nb_pixels_in: usize,
    pub nb_pixels_front: usize,
    pub nb_pixels_written: usize,
    // pub misc: String,
}

const MINIMAL_AMBIANT_LIGHT: f32 = 0.2;

fn vec_cross_z(v0: Vec3, v1: Vec3) -> f32 {
    v0.x * v1.y - v0.y * v1.x
}

fn world_to_raster(p_world: Vec3, cam: &Camera, size: PhysicalSize<u32>, ratio_w_h: f32) -> Vec3 {
    // Camera space
    let mut p = cam.world_to_sight(p_world);

    // Screen space : perspective correct
    if p.z < -0.001 {
        p.x *= cam.z_near / -p.z;
        p.y *= cam.z_near / -p.z;
    } else {
        // TODO: 0 divide getting too near the camera and reversing problem behind...
        p.x *= cam.z_near / 0.1;
        p.y *= cam.z_near / 0.1;
    };
    p.z = -p.z;

    // Near-Clipping-Plane
    // [-1,1]
    p.x /= cam.canvas_side;
    p.y /= cam.canvas_side;

    if size.width > size.height {
        p.x /= ratio_w_h;
    } else {
        p.y *= ratio_w_h;
    }

    // Raster space
    // [0,1]
    p.x = (p.x + 1.) / 2. * (size.width as f32);
    p.y = (1. - p.y) / 2. * (size.height as f32);

    p
}

fn world_to_raster_triangle(
    triangle: &Triangle,
    cam: &Camera,
    size: PhysicalSize<u32>,
    ratio_w_h: f32,
) -> Triangle {
    Triangle {
        p0: world_to_raster(triangle.p0, cam, size, ratio_w_h),
        p1: world_to_raster(triangle.p1, cam, size, ratio_w_h),
        p2: world_to_raster(triangle.p2, cam, size, ratio_w_h),
        material: triangle.material,
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
    pub max_z: f32,
}

fn bounding_box_triangle(t: &Triangle, size: PhysicalSize<u32>) -> Rect {
    Rect {
        min_x: (t.p0.x.min(t.p1.x).min(t.p2.x) as u32).clamp(0, size.width - 1),
        max_x: (t.p0.x.max(t.p1.x).max(t.p2.x) as u32).clamp(0, size.width - 1),
        min_y: (t.p0.y.min(t.p1.y).min(t.p2.y) as u32).clamp(0, size.height - 1),
        max_y: (t.p0.y.max(t.p1.y).max(t.p2.y) as u32).clamp(0, size.height - 1),
        max_z: t.p0.z.max(t.p1.z).max(t.p2.z),
    }
}

// Calculates the area of the parallelogram from vectors ab and ap
// Positive if p is "right" of ab
fn edge_function(ab: Vec3, ap: Vec3) -> f32 {
    ap.x * ab.y - ap.y * ab.x
}

fn buffer_index(p: Vec3, size: PhysicalSize<u32>) -> Option<usize> {
    if p.x >= 0. && p.x < (size.width as f32) && p.y >= 0. && p.y < (size.height as f32) {
        Some(p.x as usize + p.y as usize * size.width as usize)
    } else {
        None
    }
}

fn cursor_buffer_index(
    cursor: &Option<PhysicalPosition<f64>>,
    size: PhysicalSize<u32>,
) -> Option<usize> {
    let width = size.width as usize;
    let height = size.height as usize;
    cursor
        .filter(|c| c.x >= 0. && c.y >= 0.)
        .map(|c| (c.x as usize, c.y as usize))
        .filter(|(x, y)| *x < width && *y < height)
        .map(|(x, y)| x + y * width)
}

fn format_debug(
    settings: &Settings,
    world: &World,
    app: &AppObserver,
    size: PhysicalSize<u32>,
    cursor_color: Option<u32>,
    #[cfg(feature = "stats")] stats: &Stats,
) -> String {
    #[cfg(feature = "stats")]
    let stats = format!("{:#?}", stats);
    #[cfg(not(feature = "stats"))]
    let stats = "Stats disabled";

    // TODO: describe each numbers
    format!(
        "fps : {} | {}μs - {}μs - {}μs / {}μs / {}μs - {}μs{}\nWindow : {}x{}\nCamera : {} p: {} y: {}\n{:#?}\n{}",
        app.fps_avg().round(),
        app.last_buffer_fill_micros,
        app.last_rendering_micros,
        app.last_buffer_copy_micros,
        app.last_full_render_loop_micros(),
        app.last_frame_micros(),
        app.frame_avg_micros(),
        app.cursor()
            .and_then(|cursor| cursor_color.map(|c| format!(
                "\n({},{}) 0x{:x}",
                cursor.x.floor(),
                cursor.y.floor(),
                c
            )))
            .unwrap_or(String::from("\nNo cursor position")),
        size.width,
        size.height,
        world.camera.pos,
        world.camera.pitch,
        world.camera.yaw,
        settings,
        stats
    )
}

// TODO
/*
#[cfg(feature = "cpu")]
pub fn todo_prepare_node_single_core(
    settings: &Settings,
    triangles: &mut Vec<RenderObject>,
    view_proj: &Mat4,
    node: &Node,
) {
    {
        let world_transform = node.world_transform.borrow();
        if let Some(mesh) = node.mesh.as_ref().filter(|m| {
            todo!() || !settings.culling_meshes || m.bounds.is_visible(view_proj, &world_transform)
        }) {
            let matrix = view_proj * *world_transform;

            triangles.extend(
                mesh.surfaces
                    .iter()
                    .filter(|s| {
                        todo!()
                            || !settings.culling_surfaces
                            || s.bounds.is_visible(view_proj, &world_transform)
                    })
                    .map(|s| RenderObject {
                        vertices: mesh.indices[s.start_index..s.start_index + s.count]
                            .iter()
                            .map(|i| &mesh.vertices[*i].position)
                            .map(|v| matrix * v.extend(1.))
                            .map(|v| v.xyz() / v.w)
                            .map(|v| v.with_z(-v.z))
                            .collect(),
                        material: s.material,
                    }),
            );
        }
    }

    node.children
        .iter()
        .for_each(|c| todo_prepare_node_single_core(settings, triangles, view_proj, c));
}
*/

// TODO: group by texture to avoid duplicates ? Closer to data
#[derive(Clone, Copy)]
pub struct Triangle {
    pub p0: Vec3,
    pub p1: Vec3,
    pub p2: Vec3,
    pub material: Texture,
}

/*
impl Triangle {
    pub fn min_z(&self) -> f32 {
        f32::min(self.p0.z, f32::min(self.p1.z, self.p2.z))
    }
}
*/

pub fn populate_nodes(triangles: &mut Vec<Triangle>, node: &Node) {
    {
        if let Some(mesh) = node.mesh.as_ref() {
            let mut vertices = Vec::with_capacity(mesh.vertices.len());
            vertices.extend(
                mesh.vertices
                    .iter()
                    .map(|v| node.world_transform * v.position.extend(1.))
                    .map(|v| v.xyz()),
            );

            triangles.reserve(mesh.surfaces.iter().map(|s| s.count / 3).sum());
            triangles.extend(mesh.surfaces.iter().flat_map(|s| {
                (0..s.count)
                    .step_by(3)
                    .map(|i| s.start_index + i)
                    .map(|i| Triangle {
                        p0: vertices[mesh.indices[i]],
                        p1: vertices[mesh.indices[i + 1]],
                        p2: vertices[mesh.indices[i + 2]],
                        material: s.material,
                    })
            }));
        }
    }

    node.children
        .iter()
        .for_each(|c| populate_nodes(triangles, &c.borrow()));
}
