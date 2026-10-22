use crate::core::config::{AdapterConfig, AdapterPattern, ResolvedCrateConfig};
use crate::core::ir::{ApiSurface, EnumDef, ErrorDef, FunctionDef, MethodDef, ParamDef, TypeDef, TypeRef};
use std::collections::HashSet;

pub(crate) fn validate_surface(
    api: &ApiSurface,
    config: &ResolvedCrateConfig,
    excluded_functions: &HashSet<String>,
    excluded_types: &HashSet<String>,
    omitted_methods: &[String],
) -> anyhow::Result<()> {
    let graph = InputTypes {
        types: &api.types,
        enums: &api.enums,
        errors: &api.errors,
    };
    for function in api.functions.iter().filter(|function| {
        !excluded_functions.contains(&function.name)
            && !super::super::opaque::has_unbridgeable_param(function)
            && !config.trait_bridge_manages_function(&function.name)
    }) {
        validate_function(function, &graph)?;
    }
    validate_methods(api, excluded_types, omitted_methods, &config.adapters)
}

pub(crate) fn validate_methods(
    api: &ApiSurface,
    excluded_types: &HashSet<String>,
    omitted_methods: &[String],
    adapters: &[AdapterConfig],
) -> anyhow::Result<()> {
    let graph = InputTypes {
        types: &api.types,
        enums: &api.enums,
        errors: &api.errors,
    };
    for owner in api.types.iter().filter(|owner| {
        (owner.is_opaque || owner.is_trait) && !owner.binding_excluded && !excluded_types.contains(&owner.name)
    }) {
        for method in owner
            .methods
            .iter()
            .filter(|method| method_is_emitted(owner, method, omitted_methods, adapters))
        {
            if let Some(parameter) = graph.unsupported_parameter(&method.params) {
                anyhow::bail!(
                    concat!(
                        "cannot bridge Dart method `{}::{}` parameter `{}`: ",
                        "an error with borrowed static text cannot be used as input; ",
                        "configure dart.stub_methods or dart.exclude_types for this item"
                    ),
                    owner.name,
                    method.name,
                    parameter.name
                );
            }
        }
    }
    Ok(())
}

fn method_is_emitted(owner: &TypeDef, method: &MethodDef, omitted: &[String], adapters: &[AdapterConfig]) -> bool {
    !method.binding_excluded
        && !omitted.contains(&method.name)
        && (!method.sanitized
            || adapters.iter().any(|adapter| {
                matches!(adapter.pattern, AdapterPattern::Streaming)
                    && adapter.owner_type.as_deref() == Some(owner.name.as_str())
                    && adapter.name == method.name
            }))
}

pub(super) fn validate(function: &FunctionDef, errors: &[ErrorDef], types: &[TypeDef]) -> anyhow::Result<()> {
    validate_function(
        function,
        &InputTypes {
            types,
            enums: &[],
            errors,
        },
    )
}

fn validate_function(function: &FunctionDef, graph: &InputTypes<'_>) -> anyhow::Result<()> {
    if let Some(parameter) = graph.unsupported_parameter(&function.params) {
        anyhow::bail!(
            concat!(
                "cannot bridge Dart function `{}` parameter `{}`: ",
                "an error with borrowed static text cannot be used as input; ",
                "configure dart.exclude_functions for this function"
            ),
            function.name,
            parameter.name
        );
    }
    Ok(())
}

struct InputTypes<'a> {
    types: &'a [TypeDef],
    enums: &'a [EnumDef],
    errors: &'a [ErrorDef],
}

impl InputTypes<'_> {
    fn unsupported_parameter<'a>(&self, parameters: &'a [ParamDef]) -> Option<&'a ParamDef> {
        parameters
            .iter()
            .find(|parameter| self.contains_borrowed_error(&parameter.ty, &mut HashSet::new()))
    }

    fn contains_borrowed_error(&self, ty: &TypeRef, visited: &mut HashSet<String>) -> bool {
        match ty {
            TypeRef::Named(name) => self.named_contains_borrowed_error(name, visited),
            TypeRef::Vec(inner) | TypeRef::Optional(inner) => self.contains_borrowed_error(inner, visited),
            TypeRef::Map(key, value) => {
                self.contains_borrowed_error(key, visited) || self.contains_borrowed_error(value, visited)
            }
            _ => false,
        }
    }

    fn named_contains_borrowed_error(&self, name: &str, visited: &mut HashSet<String>) -> bool {
        if self.errors.iter().any(|error| {
            !error.binding_excluded
                && super::super::mirror::borrows_static_text(error)
                && (matches_name(name, &error.name, &error.rust_path) || name == error.original_rust_path.as_str())
        }) {
            return true;
        }
        if !visited.insert(name.to_string()) {
            return false;
        }
        if let Some(owner) = self
            .types
            .iter()
            .find(|owner| matches_name(name, &owner.name, &owner.rust_path))
        {
            return owner
                .fields
                .iter()
                .any(|field| self.contains_borrowed_error(&field.ty, visited));
        }
        self.enums
            .iter()
            .find(|owner| matches_name(name, &owner.name, &owner.rust_path))
            .is_some_and(|owner| {
                owner
                    .variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                    .any(|field| self.contains_borrowed_error(&field.ty, visited))
            })
    }
}

fn matches_name(name: &str, short: &str, path: &str) -> bool {
    name == short || name == path
}

#[cfg(test)]
mod tests {
    use crate::core::ir::{CoreWrapper, ErrorDef, ErrorVariant, FieldDef, FunctionDef, ParamDef, TypeRef};

    fn error(wrapper: CoreWrapper) -> ErrorDef {
        ErrorDef {
            name: "SampleError".into(),
            rust_path: "sample::SampleError".into(),
            original_rust_path: String::new(),
            variants: vec![ErrorVariant {
                name: "Rejected".into(),
                fields: vec![FieldDef {
                    name: "reason".into(),
                    ty: TypeRef::String,
                    core_wrapper: wrapper,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            methods: vec![],
            doc: String::new(),
            binding_excluded: false,
            binding_exclusion_reason: None,
            exception_base: None,
            version: Default::default(),
        }
    }

    #[test]
    fn bridge_rejects_borrowed_error_inputs_before_emitting_unsafe_conversion() {
        for ty in [
            TypeRef::Named("SampleError".into()),
            TypeRef::Vec(Box::new(TypeRef::Optional(Box::new(TypeRef::Named(
                "sample::SampleError".into(),
            ))))),
        ] {
            let function = FunctionDef {
                name: "consume".into(),
                params: vec![ParamDef {
                    name: "error".into(),
                    ty,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let mut generated = String::new();
            let result = super::super::emit_bridge_fn(
                &mut generated,
                &function,
                "sample",
                &Default::default(),
                &Default::default(),
                &Default::default(),
                &[],
                &[],
                &[error(CoreWrapper::StaticStr)],
            );
            assert!(generated.is_empty());
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains("consume") && message.contains("error") && message.contains("dart.exclude_functions"),
                "{message}"
            );
        }
    }

    #[test]
    fn ordinary_error_inputs_and_borrowed_error_returns_keep_their_existing_paths() {
        let mut function = FunctionDef {
            name: "consume".into(),
            params: vec![ParamDef {
                name: "error".into(),
                ty: TypeRef::Named("SampleError".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(super::validate(&function, &[error(CoreWrapper::None)], &[]).is_ok());
        function.params.clear();
        function.error_type = Some("SampleError".into());
        assert!(super::validate(&function, &[error(CoreWrapper::StaticStr)], &[]).is_ok());
    }

    #[test]
    fn borrowed_error_inputs_in_methods_and_constructors_are_rejected() {
        use crate::core::config::NewAlefConfig;
        use crate::core::ir::{ApiSurface, MethodDef, TypeDef};

        let config: NewAlefConfig = toml::from_str(
            "[workspace]\nlanguages = [\"dart\"]\n[[crates]]\nname = \"sample\"\nsources = [\"src/lib.rs\"]",
        )
        .unwrap();
        let config = config.resolve().unwrap().remove(0);
        for (name, is_static) in [("consume", false), ("new", true)] {
            let api = ApiSurface {
                types: vec![TypeDef {
                    name: "SampleClient".into(),
                    is_opaque: true,
                    methods: vec![MethodDef {
                        name: name.into(),
                        is_static,
                        params: vec![ParamDef {
                            name: "error".into(),
                            ty: TypeRef::Named("SampleError".into()),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                errors: vec![error(CoreWrapper::StaticStr)],
                ..Default::default()
            };
            let message = super::super::super::emit(&api, &config).unwrap_err().to_string();
            assert!(message.contains(&format!("SampleClient::{name}")), "{message}");
            assert!(message.contains("dart.stub_methods"), "{message}");
            assert!(super::validate_methods(&api, &Default::default(), &[name.into()], &[]).is_ok());
            assert!(
                super::validate_methods(
                    &api,
                    &std::collections::HashSet::from(["SampleClient".into()]),
                    &[],
                    &[]
                )
                .is_ok()
            );
        }
    }

    fn configuration() -> crate::core::config::ResolvedCrateConfig {
        let config: crate::core::config::NewAlefConfig = toml::from_str(
            "[workspace]\nlanguages = [\"dart\"]\n[[crates]]\nname = \"sample\"\nsources = [\"src/lib.rs\"]",
        )
        .unwrap();
        config.resolve().unwrap().remove(0)
    }

    #[test]
    fn omitted_sanitized_methods_with_borrowed_error_inputs_do_not_block_generation() {
        use crate::core::config::AdapterConfig;
        use crate::core::ir::{ApiSurface, MethodDef, TypeDef};

        let api = ApiSurface {
            types: vec![TypeDef {
                name: "SampleClient".into(),
                is_opaque: true,
                methods: vec![MethodDef {
                    name: "consume".into(),
                    sanitized: true,
                    params: vec![ParamDef {
                        name: "error".into(),
                        ty: TypeRef::Named("SampleError".into()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            errors: vec![error(CoreWrapper::StaticStr)],
            ..Default::default()
        };
        let generated = super::super::super::emit(&api, &configuration()).unwrap();
        let library = generated
            .iter()
            .find(|file| file.content.contains("pub enum SampleError"))
            .unwrap();
        assert!(!library.content.contains("pub fn consume"), "{}", library.content);
        let adapter: AdapterConfig = toml::from_str(
            "name = 'consume'\npattern = 'streaming'\ncore_path = 'consume'\nowner_type = 'SampleClient'",
        )
        .unwrap();
        assert!(super::validate_methods(&api, &Default::default(), &[], &[adapter]).is_err());
    }

    #[test]
    fn contained_borrowed_error_inputs_are_rejected_through_dto_and_enum_fields() {
        use crate::core::ir::{ApiSurface, EnumDef, EnumVariant, TypeDef};

        for container in ["Envelope", "Payload"] {
            let api = ApiSurface {
                types: vec![TypeDef {
                    name: "Envelope".into(),
                    fields: vec![FieldDef {
                        name: "payload".into(),
                        ty: TypeRef::Named("Payload".into()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                enums: vec![EnumDef {
                    name: "Payload".into(),
                    variants: vec![EnumVariant {
                        name: "Rejected".into(),
                        fields: vec![FieldDef {
                            name: "error".into(),
                            ty: TypeRef::Named("SampleError".into()),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                functions: vec![FunctionDef {
                    name: "consume".into(),
                    params: vec![ParamDef {
                        name: "value".into(),
                        ty: TypeRef::Named(container.into()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                errors: vec![error(CoreWrapper::StaticStr)],
                ..Default::default()
            };
            let message = super::super::super::emit(&api, &configuration())
                .unwrap_err()
                .to_string();
            assert!(message.contains("consume") && message.contains("value"), "{message}");
            assert!(message.contains("dart.exclude_functions"), "{message}");
        }
    }

    #[test]
    fn recursive_dto_without_borrowed_error_inputs_terminates_and_remains_supported() {
        use crate::core::ir::TypeDef;

        let types = vec![TypeDef {
            name: "Node".into(),
            fields: vec![FieldDef {
                name: "next".into(),
                ty: TypeRef::Optional(Box::new(TypeRef::Named("Node".into()))),
                ..Default::default()
            }],
            ..Default::default()
        }];
        let graph = super::InputTypes {
            types: &types,
            enums: &[],
            errors: &[error(CoreWrapper::StaticStr)],
        };
        let mut visited = std::collections::HashSet::new();
        assert!(!graph.contains_borrowed_error(&TypeRef::Named("Node".into()), &mut visited));
        assert_eq!(visited.len(), 1, "the cycle must actually be traversed once");
    }
}
