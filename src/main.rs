#![feature(slice_from_ptr_range)]
#![feature(push_mut)]
mod rasterizer;
mod window;

use window::App;

fn main() {
    App::run();
}
