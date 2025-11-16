#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// Over-print all vertices
    pub _example_setting: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            _example_setting: true,
        }
    }
}
