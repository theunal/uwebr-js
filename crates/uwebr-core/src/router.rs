/// Route definition
#[derive(Debug, Clone)]
pub struct Route {
    pub path: String,
    pub component: String,
}

/// Router state
pub struct Router {
    routes: Vec<Route>,
    current: Option<String>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: vec![],
            current: None,
        }
    }

    pub fn add_route(&mut self, path: &str, component: &str) {
        self.routes.push(Route {
            path: path.to_string(),
            component: component.to_string(),
        });
    }

    pub fn navigate(&mut self, path: &str) {
        self.current = Some(path.to_string());
    }

    pub fn current_route(&self) -> Option<&str> {
        self.current.as_deref()
    }

    pub fn resolve(&self, path: &str) -> Option<&Route> {
        self.routes.iter().find(|r| r.path == path)
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_add_and_resolve() {
        let mut router = Router::new();
        router.add_route("/", "HomePage");
        router.add_route("/about", "AboutPage");

        assert!(router.resolve("/").is_some());
        assert!(router.resolve("/about").is_some());
        assert!(router.resolve("/missing").is_none());
    }

    #[test]
    fn test_router_navigate() {
        let mut router = Router::new();
        router.add_route("/", "Home");
        router.navigate("/");
        assert_eq!(router.current_route(), Some("/"));
    }
}
