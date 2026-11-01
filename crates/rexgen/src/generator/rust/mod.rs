use crate::{generator::rust::registry::TypeRegistry, ir::Ir};

mod bindings;
mod registry;
mod types;

pub struct RustGenerator {
    ir: Ir,
    registry: TypeRegistry,
}

impl RustGenerator {
    pub fn new(ir: Ir) -> Self {
        Self {
            ir,
            registry: TypeRegistry::default(),
        }
    }
}
