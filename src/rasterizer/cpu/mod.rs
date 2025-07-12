mod cpu_engine;
mod parallel;
mod single_threaded;

use glam::{Vec3, Vec4Swizzles};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::Settings;
use crate::{
    scene::{Camera, Node, Triangle, World, world_to_raster},
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

            // triangles.reserve(mesh.surfaces.iter().map(|s| s.count).sum::<usize>() / 3);
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
        .for_each(|c| populate_nodes(triangles, &c.read().unwrap()));
}
