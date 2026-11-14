use thiserror::Error;

use crate::{
    ir::{method::Method, schema::Schema},
    spec::endpoints::ApiEndpoints,
};

pub mod field;
pub mod method;
mod resolve;
pub mod schema;

#[derive(Debug)]
pub struct Ir {
    pub endpoints: Option<ApiEndpoints>,
    pub methods: Vec<Method>,
    pub definitions: Vec<Definition>,
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub namespace: Vec<String>,
    pub anchor: String,
    pub schema: Schema,
}

#[derive(Debug, Error)]
pub enum IrError {
    #[error("anchor `{0}` is defined with different shapes")]
    AnchorShapeMismatch(String),
    #[error("unresolved $ref to anchor `{0}`")]
    UnresolvedRef(String),
}

impl Ir {
    pub fn new(endpoints: Option<ApiEndpoints>, methods: Vec<Method>) -> Result<Self, IrError> {
        let mut ir = Self {
            endpoints,
            methods,
            definitions: Vec::new(),
        };
        ir.resolve()?;
        Ok(ir)
    }
}
