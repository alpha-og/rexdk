use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use crate::{
    generator::rust::{Generator, error::RustGenerateError, registry::TypeRegistry},
    ir::method::Method,
};

impl Generator {
    pub fn generate_bindings(&mut self) -> Result<TokenStream, RustGenerateError> {
        let mut bindings = Vec::new();

        for i in 0..self.ir.methods.len() {
            let method = self.ir.methods[i].clone();
            bindings.push(self.generate_binding(&method)?);
        }

        let ty_definitions = self.registry.get_all();
        Ok(quote! {
            use crate::runtime::{RexClient, RexError};

            #(#ty_definitions)*

            #(#bindings)*
        })
    }
    fn generate_binding(&mut self, method: &Method) -> Result<TokenStream, RustGenerateError> {
        let rpc_method = &method.rpc_method;
        let method_name = method.name.to_snake_case();
        let binding_name = Ident::new(&method_name, Span::call_site());
        let description = method.description.clone();

        let arg_name = Ident::new(
            &format!("{method_name}Params").to_snake_case(),
            Span::call_site(),
        );
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

        Ok(quote! {
            #[doc = #description]
            pub async fn #binding_name(client: RexClient, #args) -> Result<#output_ty, RexError> {
                client.invoke #rpc_arg_ty(#rpc_method, #rpc_arg_name).await
            }
        })
    }
}
