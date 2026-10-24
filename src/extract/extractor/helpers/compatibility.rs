use super::attributes::cfg_attr_walk_inner_metas;

/// Read explicit constructor history without deriving it from current field order. ~keep
fn extract_legacy_constructor_fields(attrs: &[syn::Attribute]) -> anyhow::Result<Vec<String>> {
    let mut result: anyhow::Result<Option<Vec<String>>> = Ok(None);
    let mut visit = |meta: &syn::Meta| {
        let syn::Meta::List(list) = meta else { return };
        if !list.path.is_ident("alef") || result.is_err() {
            return;
        }
        result = (|| {
            let values =
                list.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)?;
            let mut found = result.as_ref().ok().cloned().flatten();
            for value in values {
                if !value.path().is_ident("legacy_constructor_fields") {
                    continue;
                }
                anyhow::ensure!(found.is_none(), "duplicate legacy_constructor_fields annotation");
                let syn::Meta::NameValue(value) = value else {
                    anyhow::bail!("legacy_constructor_fields must be an array of field names");
                };
                let syn::Expr::Array(array) = value.value else {
                    anyhow::bail!("legacy_constructor_fields must be an array of field names");
                };
                let names = array
                    .elems
                    .into_iter()
                    .map(|value| {
                        let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(name),
                            ..
                        }) = value
                        else {
                            anyhow::bail!("legacy_constructor_fields must contain string field names");
                        };
                        Ok(name.value())
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                anyhow::ensure!(!names.is_empty(), "legacy_constructor_fields must not be empty");
                found = Some(names);
            }
            Ok(found)
        })();
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

pub(crate) fn extract_constructor_metadata(
    attrs: &[syn::Attribute],
) -> anyhow::Result<(Vec<String>, Vec<Vec<String>>)> {
    Ok((
        extract_legacy_constructor_fields(attrs)?,
        super::constructor_history::extract(attrs)?,
    ))
}

/// Read the generated exception parent without evaluating cfg predicates. ~keep
pub(crate) fn extract_exception_base(attrs: &[syn::Attribute]) -> anyhow::Result<Option<String>> {
    let mut result = None;
    let mut invalid = None;
    let mut visit = |meta: &syn::Meta| {
        let syn::Meta::List(list) = meta else {
            return;
        };
        if !list.path.is_ident("alef") {
            return;
        }
        let Ok(entries) =
            list.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
        else {
            invalid = Some("invalid exception metadata");
            return;
        };
        for entry in entries {
            if !entry.path().is_ident("exception_base") {
                continue;
            }
            let syn::Meta::NameValue(value) = entry else {
                invalid = Some("exception_base must be a string parent");
                continue;
            };
            let syn::Expr::Lit(expression) = value.value else {
                invalid = Some("exception_base must be a string parent");
                continue;
            };
            let syn::Lit::Str(parent) = expression.lit else {
                invalid = Some("exception_base must be a string parent");
                continue;
            };
            if result.replace(parent.value()).is_some() {
                invalid = Some("duplicate exception_base annotation");
            }
        }
    };
    for attr in attrs {
        if attr.path().is_ident("cfg_attr") {
            cfg_attr_walk_inner_metas(attr, &mut visit);
        } else {
            visit(&attr.meta);
        }
    }
    if let Some(message) = invalid {
        anyhow::bail!("{message}");
    }
    Ok(result)
}
