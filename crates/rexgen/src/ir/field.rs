use crate::ir::schema;

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub schema: schema::Schema,
    pub required: bool,
}
