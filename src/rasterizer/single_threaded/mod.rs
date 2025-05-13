mod iterator;
mod original;

pub use iterator::IteratorEngine;
pub use original::OriginalEngine;

use std::{ops::DerefMut, time::Instant};
use winit::dpi::PhysicalSize;

use crate::{
    font::TextWriter,
    rasterizer::{Engine, Settings},
    scene::{DEFAULT_BACKGROUND_COLOR, World},
    window::AppObserver,
};

use super::{cursor_buffer_index, format_debug};

/// Common base for engines not requiring buffer synchronization.
trait SingleThreadedEngine {
    fn depth_buffer_mut(&mut self) -> &mut Vec<f32>;

    fn rasterize_world<B: DerefMut<Target = [u32]>>(
        settings: &Settings,
        world: &World,
        buffer: &mut B,
        depth_buffer: &mut [f32],
        size: PhysicalSize<u32>,
        #[cfg(feature = "stats")] stats: &mut Stats,
    );
}

impl<T: SingleThreadedEngine> Engine for T {
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
            text_writer.rasterize(buffer, size, &display[..]);
        }
    }
}

/// - Resize `depth_buffer` and fill it with inifite depth
/// - Fill `buffer` with `DEFAULT_BACKGROUND_COLOR`
/// `buffer` should be already resized.
fn clean_resize_buffers<B: DerefMut<Target = [u32]>>(
    depth_buffer: &mut Vec<f32>,
    buffer: &mut B,
    size: PhysicalSize<u32>,
) -> u128 {
    let t = Instant::now();
    buffer.fill(DEFAULT_BACKGROUND_COLOR);

    depth_buffer.resize(size.width as usize * size.height as usize, f32::INFINITY);
    depth_buffer.fill(f32::INFINITY);

    Instant::now().duration_since(t).as_micros()
}
