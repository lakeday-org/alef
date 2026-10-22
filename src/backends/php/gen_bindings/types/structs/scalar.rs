use crate::core::ir::TypeRef;
use ahash::AHashSet;

/// Returns true if the type is "scalar-compatible" — i.e. ext-php-rs can handle it as a
/// Check if a type is scalar-compatible for PHP properties, considering enum names.
/// `#[php(prop)]` without needing a manual getter.  Scalar-compatible means the mapped Rust
/// type implements `IntoZval` + `FromZval` automatically:
///   primitives, String, bool, Duration (→ u64), Path (→ String), `Option<scalar>`,
///   `Vec<primitive>` (the `Vec<T: IntoZval>` blanket impl), and — since ext-php-rs 0.15's
///   `HashMap<K, V>` conversions (`types/array/conversions/hash_map.rs`) — a String-keyed
///   `Map<String, V>` whose value type `V` is itself prop-scalar (e.g. `HashMap<String, String>`).
/// Anything containing a Named struct (that isn't an enum), a Map with a non-scalar value
/// (nested struct or `Json`), nested Vec, Json, or Bytes requires a getter.
/// Enums are mapped as String in the PHP binding, so they count as scalar.
///
/// This function is public so that `alef-e2e` can determine which fields require
/// `->getCamelCase()` getter-method syntax vs. `->camelCase` property syntax.
pub fn is_php_prop_scalar(ty: &TypeRef, enum_names: &AHashSet<String>) -> bool {
    is_php_prop_scalar_with_enums(ty, enum_names)
}

pub(super) fn is_php_prop_scalar_with_enums(ty: &TypeRef, enum_names: &AHashSet<String>) -> bool {
    match ty {
        TypeRef::Primitive(_) | TypeRef::String | TypeRef::Char | TypeRef::Duration | TypeRef::Path => true,
        TypeRef::Optional(inner) => is_php_prop_scalar_with_enums(inner, enum_names),
        TypeRef::Vec(inner) => {
            matches!(inner.as_ref(), TypeRef::Primitive(_) | TypeRef::String | TypeRef::Char)
                || matches!(inner.as_ref(), TypeRef::Named(n) if enum_names.contains(n))
        }
        TypeRef::Named(n) if enum_names.contains(n) => true,
        // `HashMap<K, V, H>` implements `IntoZval`/`FromZval` for any `K: Into<ArrayKey>` +
        // `V: IntoZval`/`FromZval` (ext-php-rs 0.15 `hash_map.rs`). Alef only models String-keyed
        // maps, so recurse on the value type; a Map of Json or a non-enum struct is
        // still not representable and falls through to the `false` arm below.
        TypeRef::Map(k, v) => matches!(k.as_ref(), TypeRef::String) && is_php_prop_scalar_with_enums(v, enum_names),
        TypeRef::Named(_) | TypeRef::Json | TypeRef::Bytes | TypeRef::Unit => false,
    }
}
