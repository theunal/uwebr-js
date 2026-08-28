/// Application events
#[derive(Debug, Clone)]
pub enum AppEvent {
    Resize(u32, u32),
    Close,
    KeyPress(String),
    MouseClick(f32, f32),
}
