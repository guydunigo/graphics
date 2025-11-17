use crate::rasterizer::vulkan::scene::Scene;

// const BASE_SCENE_NAME: &str = "basicmesh";
const SCENE_NAME: &str = "Crucru";
// const SCENE_NAME: &str = "Crucru";

const DOG_NODE_NAME: &str = "Square";

pub fn init_crucru_scene(scene: &Scene<'_>) {
    let scene = scene.loaded_scenes.get(SCENE_NAME).unwrap();
    let dog = scene.nodes.get(DOG_NODE_NAME).unwrap().borrow();
    // scene.nodes
    todo!();
}

