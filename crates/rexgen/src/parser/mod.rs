use std::collections::HashSet;

use crate::ir::Ir;
use crate::ir::method::Method;
use crate::spec::description::ApiDescription;

mod error;
mod method;
mod schema;
mod schema_kind;

pub use error::ParseError;

impl TryFrom<&ApiDescription> for Ir {
    type Error = ParseError;
    fn try_from(value: &ApiDescription) -> Result<Self, Self::Error> {
        let endpoints = None;
        let mut seen = HashSet::new();
        let mut methods = Vec::with_capacity(value.methods.len());

        for method in &value.methods {
            if !seen.insert(method.name.as_str()) {
                return Err(ParseError::DuplicateMethod(method.name.clone()));
            }

            methods.push(Method::try_from(method)?);
        }

        Ok(Ir::new(endpoints, methods)?)
    }
}
