use std::collections::{BTreeMap, btree_map::Entry};

use proc_macro2::TokenStream;
use thiserror::Error;

#[derive(Debug, Default)]
pub struct TypeRegistry {
    definitions: BTreeMap<String, TokenStream>,
}
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("type name collision: {0}")]
    NameCollision(String),
}

impl TypeRegistry {
    pub fn register(&mut self, name: &str, definition: TokenStream) -> Result<(), RegistryError> {
        match self.definitions.entry(name.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(definition);
                Ok(())
            }
            Entry::Occupied(_) => Err(RegistryError::NameCollision(name.to_string())),
        }
    }

    pub fn get_all(&self) -> Vec<&TokenStream> {
        self.definitions.values().collect()
    }
}
