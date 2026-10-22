pub(super) fn source_type(name: &str, core_import: &str) -> String {
    if name.contains("::")
        || matches!(
            name,
            "()" | "String"
                | "bool"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "u128"
                | "i128"
                | "usize"
                | "isize"
                | "char"
                | "f32"
                | "f64"
        )
    {
        name.to_string()
    } else {
        format!("{core_import}::{name}")
    }
}
