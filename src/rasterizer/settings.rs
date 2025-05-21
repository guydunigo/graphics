use super::any_engine::AnyEngine;

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// Over-print all vertices
    pub show_vertices: bool,
    /// NOTE: There might be a decoupling, it is just for testing.
    engine_type: EngineType,
    /// Sort triangles by point with mininum Z value
    ///
    /// Not implemented everywhere
    pub sort_triangles: TriangleSorting,
    pub parallel_text: bool,
    pub oversampling: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_vertices: Default::default(),
            engine_type: Default::default(),
            sort_triangles: Default::default(),
            parallel_text: true,
            oversampling: 1,
        }
    }
}

impl Settings {
    pub fn set_engine_type(&mut self, engine: &AnyEngine) {
        match engine {
            AnyEngine::Original(_) => self.engine_type = EngineType::Original,
            AnyEngine::Iterator(_) => self.engine_type = EngineType::Iterator,
            AnyEngine::ParIter2(_) => self.engine_type = EngineType::ParIter2,
            AnyEngine::ParIter3(_) => self.engine_type = EngineType::ParIter3,
            AnyEngine::ParIter4(_) => self.engine_type = EngineType::ParIter4,
            AnyEngine::ParIter5(_) => self.engine_type = EngineType::ParIter5,
            AnyEngine::Vulkan(_) => self.engine_type = EngineType::Vulkan,
        }
    }

    pub fn next_oversampling(&mut self) {
        self.oversampling = match self.oversampling {
            1 => 2,
            2 => 4,
            4 => 8,
            _ => 1,
        };
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum EngineType {
    Original,
    #[default]
    Iterator,
    ParIter2,
    ParIter3,
    ParIter4,
    ParIter5,
    Vulkan,
}

#[derive(Default, Debug, Clone, Copy)]
pub enum TriangleSorting {
    #[default]
    None,
    BackToFront,
    FrontToBack,
}

impl TriangleSorting {
    pub fn next(&mut self) {
        match self {
            TriangleSorting::None => *self = TriangleSorting::BackToFront,
            TriangleSorting::BackToFront => *self = TriangleSorting::FrontToBack,
            TriangleSorting::FrontToBack => *self = TriangleSorting::None,
        }
    }
}
