use proc_macro2::TokenStream;
use quote::quote;
use std::{
    collections::{BTreeMap, btree_map::Entry},
    path::{Path, PathBuf},
};

use crate::generator::rust::{
    Generator, RustGenerateError, bindings::GeneratedBinding, generated_file::GeneratedFile,
    namespace::ModulePath, registry::TypeRegistry,
};

#[derive(Debug, Default)]
pub struct Module {
    types: TypeRegistry,
    items: Vec<(String, TokenStream)>,
    children: BTreeMap<String, Child>,
    extra_declarations: Vec<TokenStream>,
}

#[derive(Debug)]
struct Child {
    raw: String,
    module: Module,
}

impl Module {
    pub fn declare(&mut self, declaration: TokenStream) {
        self.extra_declarations.push(declaration);
    }

    pub fn insert(
        &mut self,
        path: &ModulePath,
        generated: GeneratedBinding,
    ) -> Result<(), RustGenerateError> {
        let Some((segment, rest)) = path.0.split_first() else {
            return self.insert_item(path, generated);
        };

        let module = match self.children.entry(segment.canonical.clone()) {
            Entry::Vacant(entry) => {
                &mut entry
                    .insert(Child {
                        raw: segment.raw.clone(),
                        module: Module::default(),
                    })
                    .module
            }
            Entry::Occupied(entry) => {
                let child = entry.into_mut();

                if child.raw != segment.raw {
                    return Err(RustGenerateError::NamespaceCollision {
                        existing: child.raw.clone(),
                        incoming: segment.raw.clone(),
                        canonical: segment.canonical.clone(),
                    });
                }

                &mut child.module
            }
        };

        module.insert(&ModulePath(rest.to_vec()), generated)
    }

    fn insert_item(
        &mut self,
        path: &ModulePath,
        generated: GeneratedBinding,
    ) -> Result<(), RustGenerateError> {
        for (name, definition) in generated.types {
            self.types.register(&name, definition)?;
        }

        if self
            .items
            .iter()
            .any(|(name, _)| name == &generated.fn_name)
        {
            return Err(RustGenerateError::DuplicateBinding {
                name: generated.fn_name,
                module: path.into(),
            });
        }

        self.items.push((generated.fn_name, generated.binding));

        Ok(())
    }

    pub fn generate_files(&self, dir: &Path) -> Result<Vec<GeneratedFile>, RustGenerateError> {
        let mut files = Vec::new();
        let mut declarations = Vec::new();

        for (canonical, child) in &self.children {
            let ident = Generator::generate_identifier(canonical, "module")?;
            declarations.push(quote! {
                pub mod #ident;
            });
            files.extend(child.module.generate_files(&dir.join(canonical))?);
        }

        let items = self.items.iter().map(|(_, item)| item);
        let types = self.types.get_all();
        let extra_declarations = &self.extra_declarations;
        let has_contents = !self.items.is_empty() || !types.is_empty();
        let imports = if has_contents {
            quote! {
                use crate::runtime::{Client, Error};
            }
        } else {
            quote! {}
        };

        let definition = quote! {
            #(#extra_declarations)*
            #(#declarations)*

            #imports

            #(#types)*

            #(#items)*
        };
        let ast = syn::parse2::<syn::File>(definition)?;

        files.push(GeneratedFile {
            path: PathBuf::from(dir).join("mod.rs"),
            contents: prettyplease::unparse(&ast),
        });

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated(fn_name: &str) -> GeneratedBinding {
        let ident = Generator::generate_identifier(fn_name, "test").unwrap();

        GeneratedBinding {
            fn_name: ident.to_string(),
            types: Vec::new(),
            binding: quote! {
                pub async fn #ident() {}
            },
        }
    }

    fn path(segments: &[&str]) -> ModulePath {
        let segments = segments.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        ModulePath::try_from_segments(&segments).unwrap()
    }

    #[test]
    fn emits_nested_module_files() {
        let mut root = Module::default();
        root.insert(&path(&["session"]), generated("list")).unwrap();
        root.insert(&path(&["server", "apple"]), generated("status"))
            .unwrap();

        let files = root.generate_files(Path::new("bindings")).unwrap();
        let mut paths = files
            .iter()
            .map(|file| file.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        paths.sort();

        assert_eq!(
            paths,
            [
                "bindings/mod.rs",
                "bindings/server/apple/mod.rs",
                "bindings/server/mod.rs",
                "bindings/session/mod.rs",
            ]
        );

        for file in &files {
            syn::parse_file(&file.contents).unwrap();
        }
    }

    #[test]
    fn rejects_colliding_namespaces() {
        let mut root = Module::default();
        root.insert(&path(&["block-creator"]), generated("list"))
            .unwrap();

        let error = root
            .insert(&path(&["block_creator"]), generated("list"))
            .unwrap_err();

        assert!(matches!(
            error,
            RustGenerateError::NamespaceCollision { .. }
        ));
    }

    #[test]
    fn rejects_duplicate_bindings() {
        let mut root = Module::default();
        root.insert(&path(&["session"]), generated("list")).unwrap();

        let error = root
            .insert(&path(&["session"]), generated("list"))
            .unwrap_err();

        assert!(matches!(error, RustGenerateError::DuplicateBinding { .. }));
    }

    #[test]
    fn keeps_types_scoped_to_their_module() {
        let mut root = Module::default();
        let mut first = generated("list");
        first
            .types
            .push(("ListParams".into(), quote! { pub struct ListParams; }));
        let mut second = generated("list");
        second
            .types
            .push(("ListParams".into(), quote! { pub struct ListParams; }));

        root.insert(&path(&["client"]), first).unwrap();
        root.insert(&path(&["session"]), second).unwrap();

        let files = root.generate_files(Path::new("bindings")).unwrap();

        assert!(files.len() == 3);
    }

    #[test]
    fn skips_imports_in_empty_root() {
        let mut root = Module::default();
        root.insert(&path(&["session"]), generated("list")).unwrap();

        let files = root.generate_files(Path::new("bindings")).unwrap();
        let root = files
            .iter()
            .find(|file| file.path == Path::new("bindings/mod.rs"))
            .unwrap();

        assert!(!root.contents.contains("use crate::runtime"));
    }
}
