#![feature(slice_from_ptr_range)]
#![feature(push_mut)]
mod font;
mod maths;
mod rasterizer;
mod scene;
mod window;

use window::App;

fn main() {
    App::run();
}
