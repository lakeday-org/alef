use anyhow::Context;
use syn::{Attribute, Expr, Item, Lit, Meta, Token, punctuated::Punctuated};

pub(super) fn runtime_export_names(source: &str) -> anyhow::Result<Vec<String>> {
    let module = syn::parse_file(source).context("parsing generated NAPI bindings for runtime exports")?;
    let mut names = Vec::new();
    for item in module.items {
        let (attributes, identifier, function) = match &item {
            Item::Fn(item) => (&item.attrs, &item.sig.ident, true),
            Item::Enum(item) => (&item.attrs, &item.ident, false),
            Item::Struct(item) => (&item.attrs, &item.ident, false),
            Item::Const(item) => (&item.attrs, &item.ident, false),
            _ => continue,
        };
        let Some(options) = napi_options(attributes)? else {
            continue;
        };
        if options.iter().any(|option| option.path().is_ident("object")) {
            continue;
        }
        let rust_name = identifier.to_string();
        if let Some(namespace) = string_option(&options, "namespace") {
            names.push(namespace);
            continue;
        }
        let default_name = if function {
            crate::codegen::naming::to_node_name(&rust_name)
        } else {
            rust_name.clone()
        };
        names.push(string_option(&options, "js_name").unwrap_or(default_name));
        if matches!(item, Item::Enum(_) | Item::Struct(_)) {
            // ~keep NAPI's historical CommonJS loader also declares the Rust wrapper alias, sometimes undefined.
            names.push(rust_name);
        }
    }
    names.retain(|name| super::esm::is_esm_export_name(name));
    names.sort();
    names.dedup();
    Ok(names)
}

fn napi_options(attributes: &[Attribute]) -> anyhow::Result<Option<Punctuated<Meta, Token![,]>>> {
    let Some(attribute) = attributes.iter().find(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "napi")
    }) else {
        return Ok(None);
    };
    match &attribute.meta {
        Meta::Path(_) => Ok(Some(Punctuated::new())),
        _ => attribute
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .context("reading generated NAPI export attributes")
            .map(Some),
    }
}

fn string_option(options: &Punctuated<Meta, Token![,]>, key: &str) -> Option<String> {
    options.iter().find_map(|option| {
        let Meta::NameValue(option) = option else { return None };
        if !option.path.is_ident(key) {
            return None;
        }
        let Expr::Lit(value) = &option.value else { return None };
        let Lit::Str(value) = &value.lit else { return None };
        Some(value.value())
    })
}

#[cfg(test)]
#[path = "native_exports_tests.rs"]
mod tests;
