use winit::event::MouseButton;

/// Application events
#[derive(Debug, Clone)]
pub enum AppEvent {
    Resize(u32, u32),
    Close,
    KeyPress(String),
    KeyRelease(String),
    MouseClick(MouseButton),
    MouseRelease(MouseButton),
    MouseMove(f32, f32),
    MouseScroll(f32, f32),
}

impl AppEvent {
    pub fn name(&self) -> &'static str {
        match self {
            AppEvent::Resize(_, _) => "resize",
            AppEvent::Close => "close",
            AppEvent::KeyPress(_) => "keypress",
            AppEvent::KeyRelease(_) => "keyrelease",
            AppEvent::MouseClick(_) => "mouseclick",
            AppEvent::MouseRelease(_) => "mouserelease",
            AppEvent::MouseMove(_, _) => "mousemove",
            AppEvent::MouseScroll(_, _) => "mousescroll",
        }
    }
}
