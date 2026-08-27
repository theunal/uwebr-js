use crate::types::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub bindings: HashMap<String, Type>,
    pub parent: Option<usize>,
    pub is_async: bool,
    pub is_loop: bool,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub scopes: Vec<Scope>,
    pub current_scope: usize,
    pub functions: HashMap<String, FunctionSig>,
    pub classes: HashMap<String, StructDef>,
    pub enums: HashMap<String, EnumDef>,
    pub traits: HashMap<String, TraitDef>,
    pub return_type: Option<Type>,
    pub warnings: Vec<String>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::default()],
            current_scope: 0,
            functions: HashMap::new(),
            classes: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            return_type: None,
            warnings: Vec::new(),
        }
    }

    pub fn push_scope(&mut self) -> usize {
        let parent = self.current_scope;
        let new_scope = Scope {
            parent: Some(parent),
            is_async: self.scopes[parent].is_async,
            is_loop: self.scopes[parent].is_loop,
            ..Default::default()
        };
        self.scopes.push(new_scope);
        let idx = self.scopes.len() - 1;
        self.current_scope = idx;
        idx
    }

    pub fn pop_scope(&mut self) {
        if let Some(parent) = self.scopes[self.current_scope].parent {
            self.current_scope = parent;
        }
    }

    pub fn define_var(&mut self, name: &str, ty: Type) {
        self.scopes[self.current_scope]
            .bindings
            .insert(name.to_string(), ty);
    }

    pub fn lookup_var(&self, name: &str) -> Option<&Type> {
        let mut scope_idx = self.current_scope;
        loop {
            if let Some(ty) = self.scopes[scope_idx].bindings.get(name) {
                return Some(ty);
            }
            if let Some(parent) = self.scopes[scope_idx].parent {
                scope_idx = parent;
            } else {
                break;
            }
        }
        None
    }

    pub fn define_function(&mut self, name: &str, sig: FunctionSig) {
        self.functions.insert(name.to_string(), sig);
    }

    pub fn lookup_function(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.get(name)
    }

    pub fn define_class(&mut self, name: &str, def: StructDef) {
        self.classes.insert(name.to_string(), def);
    }

    pub fn lookup_class(&self, name: &str) -> Option<&StructDef> {
        self.classes.get(name)
    }

    pub fn is_in_loop(&self) -> bool {
        let mut scope_idx = self.current_scope;
        loop {
            if self.scopes[scope_idx].is_loop {
                return true;
            }
            if let Some(parent) = self.scopes[scope_idx].parent {
                scope_idx = parent;
            } else {
                break;
            }
        }
        false
    }

    pub fn is_in_async(&self) -> bool {
        self.scopes[self.current_scope].is_async
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }
}
