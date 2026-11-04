use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    generator::rust::{Generator, error::RustGenerateError, registry::TypeRegistry},
    ir::method::Method,
};

pub(super) struct GeneratedBinding {
    pub fn_name: String,
    pub types: Vec<(String, TokenStream)>,
    pub binding: TokenStream,
}

impl Generator {
    pub(super) fn generate_binding(
        &self,
        method: &Method,
    ) -> Result<GeneratedBinding, RustGenerateError> {
        let mut registry = TypeRegistry::default();
        let rpc_method = &method.rpc_method;
        let method_name = method.name.to_snake_case();
        let binding_name = Self::generate_identifier(&method_name, "method")?;
        let description = method.description.clone();

        let arg_name = Self::generate_identifier(&format!("{method_name}_params"), "argument")?;
        let input_ty_name = format!("{}Params", method_name.to_upper_camel_case());
        let mut rpc_arg_name = quote! {None};
        let mut rpc_arg_ty = quote! {::<(),_>};
        let args: TokenStream = match &method.input_schema {
            Some(schema) => {
                let ty = Self::generate_type(&mut registry, schema, &input_ty_name)?;
                rpc_arg_name = quote! {Some(#arg_name)};
                rpc_arg_ty = quote! {};

                quote! {
                    #arg_name: #ty
                }
            }
            None => quote! {},
        };

        let output_ty_name = format!("{}Output", method_name.to_upper_camel_case());
        let output_ty: TokenStream = match &method.output_schema {
            Some(schema) => Self::generate_type(&mut registry, schema, &output_ty_name)?,
            None => quote! {()},
        };

        let binding = quote! {
            #[doc = #description]
            pub async fn #binding_name(client: &Client, #args) -> Result<#output_ty, Error> {
                client.invoke #rpc_arg_ty(#rpc_method, #rpc_arg_name).await
            }
        };

        Ok(GeneratedBinding {
            fn_name: binding_name.to_string(),
            types: registry.flatten(),
            binding,
        })
    }
}
