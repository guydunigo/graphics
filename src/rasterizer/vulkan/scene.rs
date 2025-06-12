use std::rc::{Rc, Weak};

use glam::Mat4;

pub struct DrawContext;

trait Renderable {
    fn draw(&self, top_mat: &Mat4, ctx: &DrawContext);
}

struct Node {
    parent: Weak<Node>,
    children: Vec<Rc<Node>>,
    local_transform: Mat4,
    world_transform: Mat4,
}

impl Renderable for Node {
    fn draw(&self, top_mat: &Mat4, ctx: &DrawContext) {
        self.children.iter().for_each(|c| c.draw(top_mat, ctx));
    }
}

impl Node {
    pub fn refresh_transform(&mut self, parent_mat: &Mat4) {
        self.world_transform = parent_mat * self.local_transform;
        self.children
            .iter_mut()
            .for_each(|c| c.refresh_transform(parent_mat));
    }
}
