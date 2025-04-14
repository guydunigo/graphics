use std::{collections::HashMap, ops::DerefMut};

use fontdue::{Font, FontSettings, Metrics};
use winit::dpi::PhysicalSize;

const PX: f32 = 12.;
const BASE_X: usize = 3;
const BASE_Y: usize = 3;

pub struct TextWriter {
    font: Font,
    cache: HashMap<char, (Metrics, Vec<u8>)>,
}

impl Default for TextWriter {
    fn default() -> Self {
        let font = include_bytes!("../resources/DejaVuSansMono.ttf") as &[u8];
        Self {
            font: Font::from_bytes(font, FontSettings::default()).unwrap(),
            cache: Default::default(),
        }
    }
}

impl TextWriter {
    fn rasterize_char(&mut self, c: char) -> &(Metrics, Vec<u8>) {
        self.cache
            .entry(c)
            .or_insert_with(|| self.font.rasterize(c, PX))
    }

    pub fn rasterize<B: DerefMut<Target = [u32]>>(
        &mut self,
        buffer: &mut B,
        size: PhysicalSize<u32>,
        text: &str,
    ) {
        let mut start_y = BASE_Y;
        text.lines().for_each(|l| {
            let mut start_x = BASE_X;
            l.chars().for_each(|c| {
                let (metrics, image) = self.rasterize_char(c);

                if metrics.width > 0 {
                    image
                        .chunks(metrics.width)
                        .enumerate()
                        .flat_map(|(y, l)| l.iter().enumerate().map(move |(x, p)| (x, y, p)))
                        .for_each(|(x, y, p)| {
                            let i = metrics.xmin as isize + x as isize + start_x as isize;
                            let j = PX as isize - metrics.height as isize - metrics.ymin as isize
                                + y as isize
                                + start_y as isize;
                            if i >= 0
                                && i < size.width as isize
                                && j >= 0
                                && j < size.height as isize
                            {
                                buffer[i as usize + j as usize * size.width as usize] |=
                                    0xff000000 | ((0x00ffffff * (*p as u32)) / 255);
                            }
                        });
                }
                start_x += metrics.advance_width.ceil() as usize;
            });
            start_y += PX as usize;
        });
    }
}
