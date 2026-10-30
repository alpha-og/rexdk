use crate::ir::schema::Schema;

#[derive(Debug, Default)]
pub struct Method {
    pub name: String,
    pub description: String,
    pub input_schema: Option<Schema>,
    pub output_schema: Option<Schema>,
}
