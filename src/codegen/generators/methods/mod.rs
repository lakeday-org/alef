mod constructors;
mod impl_blocks;
mod instance;
mod opaque;
mod static_methods;
mod trait_names;

pub use constructors::{gen_constructor, gen_constructor_with_renames};
pub use impl_blocks::{gen_impl_block, gen_impl_block_with_renames};
pub use instance::gen_method;
pub use opaque::{gen_opaque_constructor, gen_opaque_impl_block};
pub use static_methods::gen_static_method;
pub use trait_names::is_trait_method_name;

fn python_export_name_attr(name: &str, cfg: &super::RustBindingConfig<'_>) -> String {
    if cfg.method_block_attr != Some("pymethods") {
        return String::new();
    }
    let exported = crate::core::keywords::python_ident(name);
    if exported == name {
        return String::new();
    }
    format!("    #[pyo3(name = \"{exported}\")]\n")
}
