mod font;
mod maths;
mod rasterizer;
mod scene;
mod window;

#[cfg(target_os = "android")]
use window::App;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    App::run_android(app);
}
