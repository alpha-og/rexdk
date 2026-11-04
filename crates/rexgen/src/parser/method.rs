use crate::{
    ir::{method::Method, schema::Schema},
    parser::error::ParseError,
    spec::method::ApiMethod,
};

impl TryFrom<&ApiMethod> for Method {
    type Error = ParseError;

    fn try_from(api_method: &ApiMethod) -> Result<Self, Self::Error> {
        let mut namespace: Vec<String> = api_method.name.split('.').map(str::to_owned).collect();
        let name = namespace.pop().ok_or(ParseError::InvalidSchema)?;

        if name.is_empty() || namespace.iter().any(String::is_empty) {
            return Err(ParseError::InvalidMethodName(api_method.name.clone()));
        }

        let input_schema = match &api_method.input_schema {
            Some(schema) => Some(Schema::try_from(schema)?),
            None => None,
        };
        let output_schema = match &api_method.output_schema {
            Some(schema) => Some(Schema::try_from(schema)?),
            None => None,
        };

        Ok(Self {
            namespace,
            name,
            description: api_method.description.clone(),
            rpc_method: api_method.invoke.method.clone(),
            input_schema,
            output_schema,
        })
    }
}
