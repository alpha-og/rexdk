use crate::{generator::rust::registry::TypeRegistry, ir::Ir};

mod bindings;
mod error;
mod registry;
pub mod types;

pub use error::RustGenerateError;

pub struct Generator {
    ir: Ir,
    registry: TypeRegistry,
}

impl Generator {
    pub fn new(ir: Ir) -> Self {
        Self {
            ir,
            registry: TypeRegistry::default(),
        }
    }
}
