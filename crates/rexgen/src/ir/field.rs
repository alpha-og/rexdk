use crate::ir::schema;

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub schema: schema::Schema,
    pub required: bool,
}
