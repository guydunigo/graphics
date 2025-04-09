mod maths;
mod scene;
// mod window;

use scene::Vec3d;
// use window::App;

fn main() {
    // App::run();
    let cam_p = Vec3d::new(1., 1., 0.);
    dbg!(cam_p);
    let z_near = -0.5;
    dbg!(z_near);
    // Squary canvas
    let canvas_side = 0.1;
    dbg!(canvas_side);

    // Pour commencer, on fixe le regard selon Z qui diminue.
    // TODO: missing double angle (autours + débullé)

    let p_world = Vec3d::new(0., 1., -10.);
    let p_cam = p_world - cam_p;
    let p_screen = Vec3d {
        x: p_cam.x * z_near / -p_cam.z,
        y: p_cam.y * z_near / -p_cam.z,
        z: -p_cam.z,
    };
    // [-1,1]
    let p_ndc = Vec3d {
        x: p_screen.x / canvas_side,
        y: p_screen.y / canvas_side,
        z: p_screen.z, // Not needed here.
    };
    // [0,1]
    let p_raster = Vec3d {
        x: (p_ndc.x + 1.) / 2.,
        y: (1. - p_ndc.y) / 2.,
        z: p_ndc.z,
    };

    dbg!(p_world);
    dbg!(p_cam);
    dbg!(p_screen);
    dbg!(p_ndc);
    dbg!(p_raster);
}
