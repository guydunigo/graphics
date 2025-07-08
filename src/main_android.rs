#![cfg(target_os = "android")]

mod font;
mod maths;
mod rasterizer;
mod scene;
mod window;

use winit::platform::android::activity::AndroidApp;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    use std::ffi::CString;

    use scene::obj_file::SUZANNE_OBJ_PATH;
    use window::App;

    // TODO: copy from assets to here ?
    eprintln!("{}", app.internal_data_path().unwrap().to_string_lossy());
    for entry in std::fs::read_dir(app.internal_data_path().unwrap()).unwrap() {
        let entry = entry.unwrap();
        println!("  - {}", entry.path().to_string_lossy());
    }
    eprintln!("{}", std::env::current_dir().unwrap().to_string_lossy());

    let assets = app.asset_manager();
    dbg!(
        assets
            .open(&CString::new("Suzanne.obj").unwrap())
            .map(|t| t.length())
    );

    App::run_android(app);
}
