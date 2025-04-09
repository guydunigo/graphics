mod maths;
mod scene;
// mod window;

use maths::Vec3f;
use scene::Triangle;
// use window::App;

fn main() {
    // App::run();
    let cam_p = Vec3f::new(1., 1., 0.);
    dbg!(cam_p);
    let z_near = -0.5;
    dbg!(z_near);
    // Squary canvas
    let canvas_side = 0.1;
    dbg!(canvas_side);

    let screen_width = 1366.;
    let screen_height = 768.;

    // Pour commencer, on fixe le regard selon Z qui diminue.
    // TODO: matrice 4x4 : missing double angle (autours + débullé)

    let triangle = Triangle::default();
    let tri_raster =
        triangle.world_to_raster(cam_p, z_near, canvas_side, screen_width, screen_height);
    dbg!(tri_raster);
}
