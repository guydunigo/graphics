mod cpu_engine;
mod parallel;
mod settings;
mod single_threaded;
mod vulkan;

use std::rc::Rc;
use vulkan::VulkanEngine;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    window::Window,
};

use crate::{
    maths::Vec3f,
    scene::{Camera, Triangle, World},
    window::AppObserver,
};

use cpu_engine::CPUEngine;
use settings::EngineType;
pub use settings::Settings;

const MINIMAL_AMBIANT_LIGHT: f32 = 0.2;

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

pub enum Engine {
    Cpu(Box<CPUEngine>),
    Vulkan(Box<VulkanEngine>),
}

impl Engine {
    pub fn new(window: Rc<Window>) -> Self {
        Self::Vulkan(Box::new(VulkanEngine::new(window)))
    }

    pub fn set_next(&mut self) {
        match self {
            Engine::Cpu(e) => {
                if e.set_next() {
                    *self = Engine::Vulkan(Box::new(VulkanEngine::new(e.window().clone())));
                }
            }
            Engine::Vulkan(e) => *self = Engine::Cpu(Box::new(CPUEngine::new(e.window().clone()))),
        }
    }

    pub fn as_engine_type(&self) -> EngineType {
        match self {
            Self::Cpu(e) => e.as_engine_type(),
            Self::Vulkan(_) => EngineType::Vulkan,
        }
    }

    pub fn rasterize(
        &mut self,
        settings: &Settings,
        world: &World,
        app: &mut AppObserver,
        #[cfg(feature = "stats")] stats: &mut Stats,
    ) {
        match self {
            Self::Cpu(e) => e.rasterize(
                settings,
                world,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
            Self::Vulkan(e) => e.rasterize(
                settings,
                world,
                app,
                #[cfg(feature = "stats")]
                stats,
            ),
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        match self {
            Self::Cpu(_) => (),
            Self::Vulkan(e) => e.on_window_event(event),
        }
    }

    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) {
        match self {
            Self::Cpu(_) => (),
            Self::Vulkan(e) => e.on_mouse_motion(delta),
        }
    }
}

fn world_to_raster(p_world: Vec3f, cam: &Camera, size: PhysicalSize<u32>, ratio_w_h: f32) -> Vec3f {
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
        texture: triangle.texture,
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
        min_x: (f32::min(f32::min(t.p0.x, t.p1.x), t.p2.x) as u32).clamp(0, size.width - 1),
        max_x: (f32::max(f32::max(t.p0.x, t.p1.x), t.p2.x) as u32).clamp(0, size.width - 1),
        min_y: (f32::min(f32::min(t.p0.y, t.p1.y), t.p2.y) as u32).clamp(0, size.height - 1),
        max_y: (f32::max(f32::max(t.p0.y, t.p1.y), t.p2.y) as u32).clamp(0, size.height - 1),
        max_z: f32::max(f32::max(t.p0.z, t.p1.z), t.p2.z),
    }
}

// Calculates the area of the parallelogram from vectors ab and ap
// Positive if p is "right" of ab
fn edge_function(ab: Vec3f, ap: Vec3f) -> f32 {
    ap.x * ab.y - ap.y * ab.x
}

fn buffer_index(p: Vec3f, size: PhysicalSize<u32>) -> Option<usize> {
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
    let cam_rot = world.camera.rot();
    #[cfg(feature = "stats")]
    let stats = format!("{:#?}", stats);
    #[cfg(not(feature = "stats"))]
    let stats = "Stats disabled";

    format!(
        "fps : {} | {}μs - {}μs - {}μs / {}μs / {}μs - {}μs{}\n{}x{}\n{} {} {} {}\n{:?}\n{}",
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
        cam_rot.u(),
        cam_rot.v(),
        cam_rot.w(),
        settings,
        stats
    )
}
