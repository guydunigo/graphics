#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// Over-print all vertices
    pub show_vertices: bool,
    /// NOTE: There might be a decoupling, it is just for testing.
    pub engine_type: EngineType,
    /// Sort triangles by point with mininum Z value
    ///
    /// Not implemented everywhere
    pub sort_triangles: TriangleSorting,
    pub parallel_text: bool,
    pub oversampling: usize,
    pub culling_mesh: bool,
    pub culling_surfaces: bool,
    pub culling_triangles: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_vertices: Default::default(),
            engine_type: Default::default(),
            sort_triangles: Default::default(),
            parallel_text: true,
            oversampling: 1,
            culling_mesh: true,
            culling_surfaces: true,
            culling_triangles: true,
        }
    }
}

impl Settings {
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
    #[default]
    None,
    Original,
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
