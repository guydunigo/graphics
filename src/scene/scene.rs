use std::{cell::RefCell, collections::HashMap, rc::Rc, thread::JoinHandle};

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
    state: WaitingOrReady,
}

impl SceneStandIn {
    pub fn new_waiting(handle: JoinHandle<Scene>) -> Self {
        Self {
            state: WaitingOrReady::Waiting(Some(handle)),
        }
    }

    pub fn new_ready(scene: Scene) -> Self {
        Self {
            state: WaitingOrReady::Ready(scene),
        }
    }

    /// Get if ready.
    pub fn get(&mut self) -> Option<&Scene> {
        if let WaitingOrReady::Waiting(handle) = &mut self.state
            && let Some(handle) = handle.take_if(|h| h.is_finished())
        {
            std::mem::replace(
                &mut self.state,
                WaitingOrReady::Ready(handle.join().unwrap()),
            );
        }

        self.state.get_ready()
    }
}

#[derive(Default)]
pub struct Scene {
    // meshes: HashMap<String, Rc<MeshAsset>>,
    named_nodes: HashMap<String, Rc<RefCell<Node>>>,

    top_nodes: Vec<Rc<RefCell<Node>>>,
}

impl Scene {
    pub fn new(
        named_nodes: HashMap<String, Rc<RefCell<Node>>>,
        top_nodes: Vec<Rc<RefCell<Node>>>,
    ) -> Self {
        // Update world transform infos to all nodes.
        top_nodes
            .iter()
            .for_each(|n| n.borrow_mut().refresh_transform(&Mat4::IDENTITY));

        Scene {
            named_nodes,
            top_nodes,
        }
    }

    pub fn top_nodes(&self) -> &[Rc<RefCell<Node>>] {
        &self.top_nodes
    }

    pub fn get_named_node(&self, name: &str) -> Option<&Rc<RefCell<Node>>> {
        self.named_nodes.get(name)
    }
}
