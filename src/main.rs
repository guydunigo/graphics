#![feature(slice_from_ptr_range)]
mod font;
mod maths;
mod rasterizer;
mod scene;
mod window;

use window::App;

fn main() {
    App::run();
}
