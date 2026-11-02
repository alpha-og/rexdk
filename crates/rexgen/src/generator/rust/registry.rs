use std::collections::BTreeMap;

use proc_macro2::{Ident, TokenStream};
use thiserror::Error;

#[derive(Default)]
pub struct TypeRegistry {
    definitions: BTreeMap<Ident, TokenStream>,
}
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("type name collision: {name}")]
    NameCollision {
        name: Ident,
        definition: TokenStream,
    },
}

impl TypeRegistry {
    pub fn register(&mut self, name: Ident, definition: TokenStream) -> Result<(), RegistryError> {
        if let Some(existing_definition) = self.definitions.insert(name.clone(), definition) {
            Err(RegistryError::NameCollision {
                name,
                definition: existing_definition,
            })
        } else {
            Ok(())
        }
    }

    pub fn get(&self, name: Ident) -> Option<&TokenStream> {
        self.definitions.get(&name)
    }

    pub fn get_all(&self) -> Vec<&TokenStream> {
        self.definitions.values().collect()
    }
}
