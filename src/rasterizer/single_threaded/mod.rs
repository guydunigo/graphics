mod iterator;
mod original;

pub use iterator::IteratorEngine;
pub use original::OriginalEngine;

use std::{ops::DerefMut, time::Instant};
use winit::dpi::PhysicalSize;

use crate::{
    font::{self, TextWriter},
    maths::{Vec3f, Vec4u},
    rasterizer::Settings,
    scene::{DEFAULT_BACKGROUND_COLOR, Texture, World},
    window::AppObserver,
};

use super::{buffer_index, cursor_buffer_index, format_debug};

#[cfg(feature = "stats")]
use crate::rasterizer::Stats;

/// Common base for engines not requiring buffer synchronization.
pub trait SingleThreadedEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32>;

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        depth_buffer: &mut [f32],
        size: PhysicalSize<u32>,
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

        let t = Instant::now();
        Self::rasterize_world(
            settings,
            world,
            buffer,
            &mut self.depth_buffer_mut()[..],
            size,
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
    v: Vec3f,
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
