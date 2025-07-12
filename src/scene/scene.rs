use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    thread::JoinHandle,
};

use glam::Mat4;

use crate::scene::Node;

enum WaitingOrReady {
    Waiting(Option<JoinHandle<Scene>>),
    Ready(Scene),
}

impl WaitingOrReady {
    fn get_ready(&self) -> Option<&Scene> {
        if let WaitingOrReady::Ready(scene) = self {
            Some(scene)
        } else {
            None
        }
    }
}

pub struct SceneStandIn {
    state: RwLock<WaitingOrReady>,
}

impl SceneStandIn {
    pub fn new_waiting(handle: JoinHandle<Scene>) -> Self {
        Self {
            state: RwLock::new(WaitingOrReady::Waiting(Some(handle))),
        }
    }

    pub fn new_ready(scene: Scene) -> Self {
        Self {
            state: RwLock::new(WaitingOrReady::Ready(scene)),
        }
    }

    // If thread is finished, read result.
    fn set_if_ready(&self) {
        let mut state = self.state.write().unwrap();
        if let WaitingOrReady::Waiting(ref mut handle) = *state
            && let Some(handle) = handle.take_if(|h| h.is_finished())
        {
            *state = WaitingOrReady::Ready(handle.join().unwrap());
        }
    }

    pub fn if_present<T>(&self, closure: impl FnOnce(&Scene) -> T) -> Option<T> {
        self.set_if_ready();

        if let WaitingOrReady::Ready(scene) = &*self.state.read().unwrap() {
            Some(closure(scene))
        } else {
            None
        }
    }
}

#[derive(Default)]
pub struct Scene {
    // meshes: HashMap<String, Arc<MeshAsset>>,
    named_nodes: HashMap<String, Arc<RwLock<Node>>>,

    top_nodes: Vec<Arc<RwLock<Node>>>,
}

impl Scene {
    pub fn new(
        named_nodes: HashMap<String, Arc<RwLock<Node>>>,
        top_nodes: Vec<Arc<RwLock<Node>>>,
    ) -> Self {
        // Update world transform infos to all nodes.
        top_nodes
            .iter()
            .for_each(|n| n.write().unwrap().refresh_transform(&Mat4::IDENTITY));

        Scene {
            named_nodes,
            top_nodes,
        }
    }

    pub fn top_nodes(&self) -> &[Arc<RwLock<Node>>] {
        &self.top_nodes
    }

    pub fn get_named_node(&self, name: &str) -> Option<&Arc<RwLock<Node>>> {
        self.named_nodes.get(name)
    }
}
