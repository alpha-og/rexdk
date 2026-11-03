use crate::{ir::method::Method, spec::endpoints::ApiEndpoints};

pub mod field;
pub mod method;
pub mod schema;

#[derive(Debug)]
pub struct Ir {
    pub endpoints: Option<ApiEndpoints>,
    pub methods: Vec<Method>,
}
