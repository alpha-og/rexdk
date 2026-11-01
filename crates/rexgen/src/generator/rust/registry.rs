use std::collections::HashMap;

use proc_macro2::{Ident, TokenStream};

#[derive(Default)]
pub struct TypeRegistry {
    definitions: HashMap<Ident, TokenStream>,
}

impl TypeRegistry {
    pub fn register(&mut self, name: Ident, definition: TokenStream) {
        self.definitions.insert(name, definition);
    }

    pub fn get(&self, name: Ident) -> Option<&TokenStream> {
        self.definitions.get(&name)
    }

    pub fn get_all(&self) -> Vec<&TokenStream> {
        self.definitions.values().collect()
    }
}
