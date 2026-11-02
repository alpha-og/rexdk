use crate::{
    ir::{method::Method, schema::Schema},
    parser::error::ParseError,
    spec::method::ApiMethod,
};

impl TryFrom<&ApiMethod> for Method {
    type Error = ParseError;

    fn try_from(api_method: &ApiMethod) -> Result<Self, Self::Error> {
        let input_schema = match &api_method.input_schema {
            Some(schema) => Some(Schema::try_from(schema)?),
            None => None,
        };
        let output_schema = match &api_method.output_schema {
            Some(schema) => Some(Schema::try_from(schema)?),
            None => None,
        };

        Ok(Self {
            name: api_method.name.clone(),
            description: api_method.description.clone(),
            rpc_method: api_method.invoke.method.clone(),
            input_schema,
            output_schema,
        })
    }
}
