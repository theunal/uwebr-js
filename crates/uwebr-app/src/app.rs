use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use uwebr_core::events::dispatch_action;
use uwebr_core::signal::take_render_dirty;
use uwebr_core::timer::timer_registry;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{WindowAttributes, WindowId};

use crate::component::Component;
use crate::context::GpuContext;
use crate::event::AppEvent;
use crate::pipeline::RenderPipeline;
use crate::window::Window;
use uwebr_render::stylebook::StyleBook;

/// Per-window state
struct WindowState {
    ctx: GpuContext,
    pipeline: RenderPipeline,
    component: Option<Box<dyn Component>>,
    /// Latest cursor position in physical pixels, for click hit-testing.
    cursor: (f32, f32),
    /// Layout node currently under the cursor, driving `:hover`.
    hovered_element: Option<usize>,
}

impl WindowState {
    fn new(ctx: GpuContext, component: Option<Box<dyn Component>>) -> Self {
        Self {
            ctx,
            pipeline: RenderPipeline::new(),
            component,
            cursor: (0.0, 0.0),
            hovered_element: None,
        }
    }

    fn render(&mut self) {
        if let Some(ref component) = self.component {
            let element = component.render();
            let (w, h) = self.ctx.size();
            let scene = self.pipeline.render(&element, w, h);
            if let Err(e) = self.ctx.render_scene(&scene) {
                eprintln!("Render error: {e}");
            }
        }
    }

    /// Route a click at the current cursor to the registered action, if any.
    ///
    /// Returns true when a handler ran, so the caller can request a redraw.
    fn handle_click(&mut self) -> bool {
        let (x, y) = self.cursor;
        match self.pipeline.hit_test(x, y) {
            Some(action) => {
                let action = action.to_string();
                dispatch_action(&action)
            }
            None => false,
        }
    }

    /// Update `:hover` state from the current cursor position.
    ///
    /// Returns true when the hovered element changed, so the caller can request
    /// a redraw to reflect any `:hover` rules.
    fn update_hover(&mut self) -> bool {
        let (x, y) = self.cursor;
        let new_hovered = self.pipeline.hit_test_hover(x, y);
        if new_hovered == self.hovered_element {
            return false;
        }
        if let Some(old) = self.hovered_element {
            uwebr_core::state::set_hovered(old, false);
        }
        if let Some(new) = new_hovered {
            uwebr_core::state::set_hovered(new, true);
        }
        self.hovered_element = new_hovered;
        true
    }
}

/// Boxed listener for generic application events.
type EventListener = Box<dyn Fn(&AppEvent) + Send + 'static>;

/// A window queued for creation on the next `resumed`.
///
/// Winit only hands out an `ActiveEventLoop` inside handlers, so windows
/// requested before `run()` must be parked here.
type PendingWindow = (String, u32, u32, Option<Box<dyn Component>>);

/// Main application entry point with multi-window support
pub struct App {
    title: String,
    width: u32,
    height: u32,
    component: Option<Box<dyn Component>>,
    event_handlers: Vec<EventListener>,
    windows: HashMap<WindowId, WindowState>,
    primary_window: Option<WindowId>,
    pending_windows: Vec<PendingWindow>,
    stylebook: Option<StyleBook>,
    /// Raw CSS kept so windows can re-resolve `vw`/`vh` on resize.
    css_string: Option<String>,
}

impl App {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            width: 800,
            height: 600,
            component: None,
            event_handlers: vec![],
            windows: HashMap::new(),
            primary_window: None,
            pending_windows: vec![],
            stylebook: None,
            css_string: None,
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

    /// Load CSS rules — parsed into StyleBook and applied to all windows
    pub fn with_css(mut self, css: &str) -> Self {
        if let Ok(sb) = StyleBook::parse(css) {
            self.stylebook = Some(sb);
        }
        self.css_string = Some(css.to_string());
        self
    }

    /// Set StyleBook directly (pre-parsed CSS rules)
    pub fn with_stylebook(mut self, stylebook: StyleBook) -> Self {
        self.stylebook = Some(stylebook);
        self
    }

    pub fn on_event<F: Fn(&AppEvent) + Send + 'static>(mut self, handler: F) -> Self {
        self.event_handlers.push(Box::new(handler));
        self
    }

    /// Open a new window (can be called before run())
    pub fn open_window(
        mut self,
        title: &str,
        width: u32,
        height: u32,
        component: impl Component,
    ) -> Self {
        self.pending_windows
            .push((title.to_string(), width, height, Some(Box::new(component))));
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

    /// Get a Window wrapper for a specific window
    fn get_window(&self, id: WindowId) -> Option<Window> {
        self.windows
            .get(&id)
            .map(|ws| Window::from_winit(ws.ctx.window.clone()))
    }

    /// Get the primary window
    pub fn primary_window(&self) -> Option<Window> {
        self.primary_window.and_then(|id| self.get_window(id))
    }

    /// Number of open windows
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Number of pending windows (not yet created)
    pub fn pending_window_count(&self) -> usize {
        self.pending_windows.len()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new("uwebr App")
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
        timer_registry().tick();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create primary window on first resume
        if self.primary_window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title(&self.title)
                .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height));

            let window = match event_loop.create_window(attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    eprintln!("Failed to create primary window: {e}");
                    event_loop.exit();
                    return;
                }
            };

            let ctx = match pollster::block_on(GpuContext::new(window)) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to initialize GPU for primary window: {e}");
                    event_loop.exit();
                    return;
                }
            };

            let id = ctx.window().id();
            self.primary_window = Some(id);
            let mut ws = WindowState::new(ctx, self.component.take());
            if let Some(ref sb) = self.stylebook {
                ws.pipeline = ws.pipeline.with_stylebook(sb.clone());
            }
            if let Some(ref css) = self.css_string {
                ws.pipeline = ws.pipeline.with_css_source(css);
            }
            self.windows.insert(id, ws);
        }

        // Create pending windows
        let pending = std::mem::take(&mut self.pending_windows);
        for (title, width, height, component) in pending {
            let attrs = WindowAttributes::default()
                .with_title(&title)
                .with_inner_size(winit::dpi::LogicalSize::new(width, height));

            let window = match event_loop.create_window(attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    eprintln!("Failed to create window '{title}': {e}");
                    continue;
                }
            };

            let ctx = match pollster::block_on(GpuContext::new(window)) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to initialize GPU for window '{title}': {e}");
                    continue;
                }
            };

            let id = ctx.window().id();
            let mut ws = WindowState::new(ctx, component);
            if let Some(ref sb) = self.stylebook {
                ws.pipeline = ws.pipeline.with_stylebook(sb.clone());
            }
            if let Some(ref css) = self.css_string {
                ws.pipeline = ws.pipeline.with_css_source(css);
            }
            self.windows.insert(id, ws);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                self.windows.remove(&id);
                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                state.ctx.resize(size.width, size.height);
                self.dispatch_event(&AppEvent::Resize(size.width, size.height));
            }
            WindowEvent::RedrawRequested => {
                timer_registry().fire_animation_frames();
                state.render();
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor = (position.x as f32, position.y as f32);
                // Recompute `:hover`; a change requires a repaint to apply any
                // hover rules to the newly (un)hovered element.
                if state.update_hover() {
                    state.ctx.window().request_redraw();
                }
                self.dispatch_event(&AppEvent::MouseMove(position.x as f32, position.y as f32));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.dispatch_event(&AppEvent::KeyPress(format!("{:?}", event.logical_key)));
            }
            WindowEvent::MouseInput {
                state: input_state,
                button,
                ..
            } if input_state.is_pressed() => {
                // Route the click to an `on:click` handler before notifying
                // generic listeners; the handler may mutate state.
                if button == winit::event::MouseButton::Left && state.handle_click() {
                    state.ctx.window().request_redraw();
                }
                self.dispatch_event(&AppEvent::MouseClick(button));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // winit: positive LineDelta(y) = scroll up → negate so positive = scroll content down
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 20.0, -y * 20.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        (-(pos.x as f32), -(pos.y as f32))
                    }
                };
                state.pipeline.scroll_by(dx, dy);
                state.ctx.window().request_redraw();
                self.dispatch_event(&AppEvent::MouseScroll(dx, dy));
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Two independent repaint sources:
        //   1. pending timers (setTimeout/setInterval/requestAnimationFrame)
        //   2. signal writes — state changed, so the rendered tree is stale
        let registry = timer_registry();
        let state_changed = take_render_dirty();

        if registry.has_pending() || state_changed {
            for state in self.windows.values() {
                state.ctx.window().request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_multi_window_fields() {
        let app = App::new("Test");
        assert_eq!(app.window_count(), 0);
        assert!(app.primary_window.is_none());
    }

    #[test]
    fn test_app_with_pending_window() {
        let app = App::new("Test").open_window(
            "Child",
            400,
            300,
            crate::FnComponent::new(|| uwebr_core::component::Element {
                node_type: uwebr_core::component::NodeType::Element("div".into()),
                props: vec![],
                children: vec![],
            }),
        );
        assert_eq!(app.pending_window_count(), 1);
    }

    #[test]
    fn test_app_multiple_pending_windows() {
        use crate::FnComponent;
        use uwebr_core::component::{Element, NodeType};

        let app = App::new("Main")
            .open_window(
                "Win1",
                400,
                300,
                FnComponent::new(|| Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![],
                    children: vec![],
                }),
            )
            .open_window(
                "Win2",
                600,
                400,
                FnComponent::new(|| Element {
                    node_type: NodeType::Element("span".into()),
                    props: vec![],
                    children: vec![],
                }),
            );
        assert_eq!(app.pending_window_count(), 2);
    }

    #[test]
    fn test_app_window_count_empty() {
        let app = App::new("Test");
        assert_eq!(app.window_count(), 0);
    }

    #[test]
    fn test_app_primary_window_none_before_run() {
        let app = App::new("Test");
        assert!(app.primary_window().is_none());
    }
}
