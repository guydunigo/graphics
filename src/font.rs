#[cfg(feature = "vulkan")]
pub const FONT_NAME: &str = "DejaVuSansMono";
pub const FONT: &[u8] = include_bytes!("../resources/DejaVuSansMono.ttf") as &[u8];

#[cfg(feature = "cpu")]
pub use cpu::*;

#[cfg(feature = "cpu")]
mod cpu {
    use std::sync::atomic::{AtomicU64, Ordering};

    use fontdue::{Font, FontSettings};
    use rayon::prelude::*;
    use winit::dpi::PhysicalSize;

    use super::FONT;

    pub const PX: f32 = 12.;
    const BASE_X: usize = 3;
    const BASE_Y: usize = 3;

    #[derive(Debug, Clone)]
    pub struct TextWriter {
        font: Font,
    }

    impl Default for TextWriter {
        fn default() -> Self {
            Self {
                font: Font::from_bytes(FONT, FontSettings::default()).unwrap(),
            }
        }
    }

    impl TextWriter {
        fn rasterize_generic(
            &self,
            size: PhysicalSize<u32>,
            px: f32,
            i: usize,
            l: &str,
            mut set_buffer: impl FnMut(usize, u32),
        ) {
            let mut start_x = BASE_X;
            let start_y = BASE_Y + i * px as usize;

            l.chars().for_each(|c| {
                let (metrics, image) = self.font.rasterize(c, px);

                if metrics.width > 0 {
                    image
                        .chunks(metrics.width)
                        .enumerate()
                        .flat_map(|(y, l)| l.iter().enumerate().map(move |(x, p)| (x, y, p)))
                        .filter(|(_, _, p)| **p != 0)
                        .for_each(|(x, y, p)| {
                            let i = metrics.xmin as isize + x as isize + start_x as isize;
                            let j = px as isize - metrics.height as isize - metrics.ymin as isize
                                + y as isize
                                + start_y as isize;
                            if i >= 0
                                && i < size.width as isize
                                && j >= 0
                                && j < size.height as isize
                            {
                                set_buffer(
                                    i as usize + j as usize * size.width as usize,
                                    0xff000000 | ((0x00ffffff * (*p as u32)) / 255),
                                );
                            }
                        });
                }
                start_x += metrics.advance_width.ceil() as usize;
            });
        }

        pub fn rasterize(&self, buffer: &mut [u32], size: PhysicalSize<u32>, px: f32, text: &str) {
            text.lines().enumerate().for_each(|(i, l)| {
                self.rasterize_generic(size, px, i, l, |index, color| buffer[index] |= color);
            });
        }

        pub fn rasterize_par(
            &self,
            buffer: &[AtomicU64],
            size: PhysicalSize<u32>,
            px: f32,
            text: &str,
        ) {
            text.lines().enumerate().par_bridge().for_each(|(i, l)| {
                self.rasterize_generic(size, px, i, l, |index, color| {
                    buffer[index].fetch_or(color as u64, Ordering::Relaxed);
                });
            });
        }
    }
}
