use anyhow::Result;
use std::sync::Arc;
use uwebr_render::layout::LayoutEngine;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{WindowAttributes, WindowId};

use crate::component::Component;
use crate::context::GpuContext;
use crate::event::AppEvent;

/// Application state
enum AppState {
    /// Waiting for GPU context initialization
    WaitingResume,
    /// GPU context is ready
    Running {
        ctx: GpuContext,
        layout_engine: LayoutEngine,
    },
}

/// Main application entry point
pub struct App {
    title: String,
    width: u32,
    height: u32,
    component: Option<Box<dyn Component>>,
    event_handlers: Vec<Box<dyn Fn(&AppEvent) + Send + 'static>>,
    state: AppState,
}

impl App {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            width: 800,
            height: 600,
            component: None,
            event_handlers: vec![],
            state: AppState::WaitingResume,
        }
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_component(mut self, component: impl Component) -> Self {
        self.component = Some(Box::new(component));
        self
    }

    pub fn on_event<F: Fn(&AppEvent) + Send + 'static>(mut self, handler: F) -> Self {
        self.event_handlers.push(Box::new(handler));
        self
    }

    fn dispatch_event(&self, event: &AppEvent) {
        for handler in &self.event_handlers {
            handler(event);
        }
    }

    /// Run the application event loop (blocking)
    pub fn run(self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        let mut app = self;
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new("uwebr App")
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {}

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, AppState::WaitingResume) {
            // Create window
            let attrs = WindowAttributes::default()
                .with_title(&self.title)
                .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height));

            let window = match event_loop.create_window(attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    eprintln!("Failed to create window: {e}");
                    event_loop.exit();
                    return;
                }
            };

            // Initialize GPU context
            let ctx = match pollster::block_on(GpuContext::new(window)) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to initialize GPU: {e}");
                    event_loop.exit();
                    return;
                }
            };

            self.state = AppState::Running {
                ctx,
                layout_engine: LayoutEngine::new(),
            };
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match &mut self.state {
            AppState::WaitingResume => {}
            AppState::Running { ctx, layout_engine } => {
                match event {
                    WindowEvent::CloseRequested => {
                        event_loop.exit();
                    }
                    WindowEvent::Resized(size) => {
                        ctx.resize(size.width, size.height);
                        self.dispatch_event(&AppEvent::Resize(size.width, size.height));
                    }
                    WindowEvent::RedrawRequested => {
                        // Build element tree from component
                        if let Some(ref component) = self.component {
                            let element = component.render();

                            // Layout
                            let _ = layout_engine.build_tree(&element);
                            // (Full pipeline: layout → scene → render — see FAZ 4 integration)

                            // For now, just clear with black
                            let scene = vello::Scene::new();
                            if let Err(e) = ctx.render_scene(&scene) {
                                eprintln!("Render error: {e}");
                            }
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        self.dispatch_event(&AppEvent::KeyPress(
                            format!("{:?}", event.logical_key),
                        ));
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if state.is_pressed() {
                            self.dispatch_event(&AppEvent::MouseClick(
                                button,
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let AppState::Running { ref ctx, .. } = self.state {
            ctx.window().request_redraw();
        }
    }
}
