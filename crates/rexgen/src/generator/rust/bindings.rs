use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};

use crate::{generator::rust::RustGenerator, ir::method::Method};

impl RustGenerator {
    pub fn generate_bindings(&mut self) -> TokenStream {
        let mut bindings = Vec::new();

        for i in 0..self.ir.methods.len() {
            let method = self.ir.methods[i].clone();
            bindings.push(self.generate_binding(&method));
        }

        let ty_definitions = self.registry.get_all();
        quote! {
            #(#ty_definitions)*

            #(#bindings)*
        }
    }
    fn generate_binding(&mut self, method: &Method) -> TokenStream {
        let method_name = method.name.to_upper_camel_case();
        let binding_name = Ident::new(&method_name, Span::call_site());
        let description = method.description.clone();

        let arg_name = Ident::new(
            &format!("{method_name}Params").to_snake_case(),
            Span::call_site(),
        );
        let input_ty_name = format_ident!("{binding_name}Params");
        let args: TokenStream = match &method.input_schema {
            Some(schema) => {
                let ty: TokenStream = self.generate_type(&schema, input_ty_name);
                quote! {
                    #arg_name: #ty
                }
            }
            None => quote! {},
        };

        let output_ty_name = format_ident!("{binding_name}Output");
        let output_ty: TokenStream = match &method.output_schema {
            Some(schema) => {
                let ty: TokenStream = self.generate_type(&schema, output_ty_name);
                quote! {
                    #ty
                }
            }
            None => quote! {()},
        };

        quote! {
            #[doc = #description]
            fn #binding_name(#args) -> anyhow::Result<#output_ty> {
            }
        }
    }
}
