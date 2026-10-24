use super::attributes::cfg_attr_walk_inner_metas;

pub(super) fn extract(attrs: &[syn::Attribute]) -> anyhow::Result<Vec<Vec<String>>> {
    let mut result: anyhow::Result<Option<Vec<Vec<String>>>> = Ok(None);
    let mut visit = |meta: &syn::Meta| {
        let syn::Meta::List(list) = meta else { return };
        if !list.path.is_ident("alef") || result.is_err() {
            return;
        }
        result = read_history(list, result.as_ref().ok().cloned().flatten());
    };
    for attr in attrs {
        if attr.path().is_ident("cfg_attr") {
            cfg_attr_walk_inner_metas(attr, &mut visit);
        } else {
            visit(&attr.meta);
        }
    }
    Ok(result?.unwrap_or_default())
}

fn read_history(list: &syn::MetaList, mut found: Option<Vec<Vec<String>>>) -> anyhow::Result<Option<Vec<Vec<String>>>> {
    let entries = list.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)?;
    for entry in entries {
        if !entry.path().is_ident("constructor_history") {
            continue;
        }
        anyhow::ensure!(found.is_none(), "duplicate constructor_history annotation");
        let syn::Meta::NameValue(value) = entry else {
            anyhow::bail!("constructor_history must be an array of field-name arrays");
        };
        let syn::Expr::Array(array) = value.value else {
            anyhow::bail!("constructor_history must be an array of field-name arrays");
        };
        anyhow::ensure!(!array.elems.is_empty(), "constructor_history must not be empty");
        found = Some(
            array
                .elems
                .into_iter()
                .map(read_signature)
                .collect::<anyhow::Result<_>>()?,
        );
    }
    Ok(found)
}

fn read_signature(expression: syn::Expr) -> anyhow::Result<Vec<String>> {
    let syn::Expr::Array(array) = expression else {
        anyhow::bail!("constructor_history signatures must be arrays of field names");
    };
    array
        .elems
        .into_iter()
        .map(|expression| {
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(name),
                ..
            }) = expression
            else {
                anyhow::bail!("constructor_history signatures must contain string field names");
            };
            Ok(name.value())
        })
        .collect()
}
