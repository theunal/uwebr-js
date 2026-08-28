use uwebr_core::component::Element;

/// A UI component that can be rendered by the app
pub trait Component: Send + 'static {
    /// Render the component to an Element tree
    fn render(&self) -> Element;
}

/// A closure-based component
pub struct FnComponent {
    render_fn: Box<dyn Fn() -> Element + Send + 'static>,
}

impl FnComponent {
    /// Create a component from a closure
    pub fn new<F: Fn() -> Element + Send + 'static>(f: F) -> Self {
        Self {
            render_fn: Box::new(f),
        }
    }
}

impl Component for FnComponent {
    fn render(&self) -> Element {
        (self.render_fn)()
    }
}
