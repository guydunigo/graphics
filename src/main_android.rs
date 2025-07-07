mod font;
mod maths;
mod rasterizer;
mod scene;
mod window;

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
use window::App;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    eprintln!("{}", std::env::current_dir().unwrap().to_string_lossy());

    App::run_android(app);
}
