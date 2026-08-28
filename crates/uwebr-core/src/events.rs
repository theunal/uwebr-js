use std::collections::HashMap;
use std::rc::Rc;

/// Event types supported by the framework
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    Click,
    DoubleClick,
    MouseDown,
    MouseUp,
    MouseEnter,
    MouseLeave,
    MouseMove,
    KeyDown,
    KeyUp,
    Input,
    Change,
    Focus,
    Blur,
    Submit,
    Custom(String),
}

impl EventType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "click" | "on:click" => Self::Click,
            "dblclick" | "on:dblclick" => Self::DoubleClick,
            "mousedown" | "on:mousedown" => Self::MouseDown,
            "mouseup" | "on:mouseup" => Self::MouseUp,
            "mouseenter" | "on:mouseenter" => Self::MouseEnter,
            "mouseleave" | "on:mouseleave" => Self::MouseLeave,
            "mousemove" | "on:mousemove" => Self::MouseMove,
            "keydown" | "on:keydown" => Self::KeyDown,
            "keyup" | "on:keyup" => Self::KeyUp,
            "input" | "on:input" => Self::Input,
            "change" | "on:change" => Self::Change,
            "focus" | "on:focus" => Self::Focus,
            "blur" | "on:blur" => Self::Blur,
            "submit" | "on:submit" => Self::Submit,
            other => Self::Custom(other.to_string()),
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Self::Click => "click",
            Self::DoubleClick => "dblclick",
            Self::MouseDown => "mousedown",
            Self::MouseUp => "mouseup",
            Self::MouseEnter => "mouseenter",
            Self::MouseLeave => "mouseleave",
            Self::MouseMove => "mousemove",
            Self::KeyDown => "keydown",
            Self::KeyUp => "keyup",
            Self::Input => "input",
            Self::Change => "change",
            Self::Focus => "focus",
            Self::Blur => "blur",
            Self::Submit => "submit",
            Self::Custom(s) => s,
        }
    }
}

/// Event data carrying payload
#[derive(Debug, Clone)]
pub struct Event {
    pub event_type: EventType,
    pub target: Option<u64>,
    pub data: EventData,
}

/// Event-specific data
#[derive(Debug, Clone)]
pub enum EventData {
    Mouse { x: f64, y: f64, button: MouseButton },
    Keyboard { key: String, code: u32, modifiers: Modifiers },
    Input { value: String },
    Focus,
    Submit { form_data: HashMap<String, String> },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Event handler function type
pub type EventHandler = Rc<dyn Fn(&Event)>;

/// Event dispatcher: manages event listeners per (target_id, event_type)
pub struct EventDispatcher {
    listeners: HashMap<(u64, EventType), Vec<EventHandler>>,
    next_id: u64,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register an event listener, returns a listener ID for removal
    pub fn on(&mut self, target_id: u64, event_type: EventType, handler: EventHandler) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.listeners
            .entry((target_id, event_type))
            .or_default()
            .push(handler);
        id
    }

    /// Remove a specific listener by ID (simplified: removes all for that target+type)
    pub fn off(&mut self, target_id: u64, event_type: &EventType) {
        self.listeners.remove(&(target_id, event_type.clone()));
    }

    /// Dispatch an event to all registered handlers
    pub fn dispatch(&self, event: &Event) {
        let key = (event.target.unwrap_or(0), event.event_type.clone());
        if let Some(handlers) = self.listeners.get(&key) {
            for handler in handlers {
                handler(event);
            }
        }
    }

    /// Remove all listeners
    pub fn clear(&mut self) {
        self.listeners.clear();
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn test_event_type_from_str() {
        assert_eq!(EventType::from_str("click"), EventType::Click);
        assert_eq!(EventType::from_str("on:click"), EventType::Click);
        assert_eq!(EventType::from_str("input"), EventType::Input);
        assert_eq!(EventType::from_str("on:input"), EventType::Input);
        assert_eq!(
            EventType::from_str("custom"),
            EventType::Custom("custom".to_string())
        );
    }

    #[test]
    fn test_event_dispatch() {
        let mut dispatcher = EventDispatcher::new();
        let clicked = Rc::new(Cell::new(false));
        let clicked_clone = clicked.clone();

        dispatcher.on(
            1,
            EventType::Click,
            Rc::new(move |_event| {
                clicked_clone.set(true);
            }),
        );

        let event = Event {
            event_type: EventType::Click,
            target: Some(1),
            data: EventData::Mouse {
                x: 100.0,
                y: 200.0,
                button: MouseButton::Left,
            },
        };

        dispatcher.dispatch(&event);
        assert!(clicked.get());
    }

    #[test]
    fn test_event_no_handler() {
        let dispatcher = EventDispatcher::new();
        let event = Event {
            event_type: EventType::Click,
            target: Some(999),
            data: EventData::None,
        };
        // Should not panic
        dispatcher.dispatch(&event);
    }

    #[test]
    fn test_event_off() {
        let mut dispatcher = EventDispatcher::new();
        let called = Rc::new(Cell::new(false));
        let called_clone = called.clone();

        dispatcher.on(
            1,
            EventType::Click,
            Rc::new(move |_| {
                called_clone.set(true);
            }),
        );

        dispatcher.off(1, &EventType::Click);

        let event = Event {
            event_type: EventType::Click,
            target: Some(1),
            data: EventData::None,
        };
        dispatcher.dispatch(&event);
        assert!(!called.get());
    }

    #[test]
    fn test_multiple_handlers() {
        let mut dispatcher = EventDispatcher::new();
        let count = Rc::new(Cell::new(0));

        for _ in 0..3 {
            let count_clone = count.clone();
            dispatcher.on(
                1,
                EventType::Click,
                Rc::new(move |_| {
                    count_clone.set(count_clone.get() + 1);
                }),
            );
        }

        let event = Event {
            event_type: EventType::Click,
            target: Some(1),
            data: EventData::None,
        };
        dispatcher.dispatch(&event);
        assert_eq!(count.get(), 3);
    }

    #[test]
    fn test_modifiers() {
        let mods = Modifiers {
            shift: true,
            ctrl: false,
            alt: true,
            meta: false,
        };
        assert!(mods.shift);
        assert!(!mods.ctrl);
        assert!(mods.alt);
        assert!(!mods.meta);
    }
}
