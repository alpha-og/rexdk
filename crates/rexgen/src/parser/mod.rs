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
        let endpoints = value.http.clone();
        let methods = value
            .methods
            .iter()
            .map(Method::try_from)
            .collect::<Result<Vec<_>, Self::Error>>()?;
        Ok(Self { endpoints, methods })
    }
}
