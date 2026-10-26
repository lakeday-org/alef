use super::{embedding_input_enum, empty_api, function_taking, make_config_with_text_types};
use crate::backends::wasm::gen_bindings::WasmBackend;
use crate::core::backend::Backend;
use crate::core::config::ResolvedCrateConfig;
use crate::core::ir::{ApiSurface, EnumDef, FieldDef, TypeDef, TypeRef};
use quote::ToTokens;

fn moderation_api(text_union: EnumDef) -> ApiSurface {
    let mut api = empty_api();
    api.enums = vec![text_union];
    api.functions = vec![function_taking("ModerationRequest")];
    api.types = vec![TypeDef {
        name: "ModerationRequest".to_string(),
        rust_path: "test_lib::ModerationRequest".to_string(),
        fields: vec![FieldDef {
            name: "input".to_string(),
            ty: TypeRef::Named("EmbeddingInput".to_string()),
            optional: true,
            ..Default::default()
        }],
        has_serde: true,
        ..Default::default()
    }];
    api
}

fn generated_lib_rs(api: &ApiSurface, config: &ResolvedCrateConfig) -> String {
    let files = WasmBackend.generate_bindings(api, config).unwrap();
    files
        .iter()
        .find(|f| f.path.to_string_lossy().ends_with("lib.rs"))
        .expect("lib.rs must be generated")
        .content
        .clone()
}

#[test]
fn serializable_text_union_field_gets_the_companion_and_getter() {
    let api = moderation_api(embedding_input_enum());
    let lib_rs = generated_lib_rs(&api, &make_config_with_text_types("\"EmbeddingInput\""));

    assert!(
        lib_rs.contains("input_value: Option<JsValue>,"),
        "the struct must keep the whole union beside the display text;\nactual:\n{lib_rs}"
    );
    assert!(
        lib_rs.contains("js_name = \"inputValue\""),
        "the lossless getter must be emitted;\nactual:\n{lib_rs}"
    );
}

#[test]
fn non_serializable_text_union_field_gets_no_companion() {
    let mut union = embedding_input_enum();
    union.has_serde = false;
    let api = moderation_api(union);
    let lib_rs = generated_lib_rs(&api, &make_config_with_text_types("\"EmbeddingInput\""));

    assert!(
        lib_rs.contains("input: Option<String>,"),
        "the field stays display text;\nactual:\n{lib_rs}"
    );
    assert!(
        !lib_rs.contains("input_value") && !lib_rs.contains("inputValue"),
        "no companion without Serialize;\nactual:\n{lib_rs}"
    );
}

#[test]
fn serializable_text_union_core_conversion_populates_the_companion() {
    let api = moderation_api(embedding_input_enum());
    let lib_rs = generated_lib_rs(&api, &make_config_with_text_types("\"EmbeddingInput\""));
    let parsed: syn::File = syn::parse_str(&lib_rs).expect("generated bindings must be valid Rust");
    let conversion = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(implementation) => Some(implementation),
            _ => None,
        })
        .find(|implementation| {
            implementation.self_ty.to_token_stream().to_string() == "WasmModerationRequest"
                && implementation.trait_.as_ref().is_some_and(|(path, _)| {
                    path.to_token_stream().to_string() == "From < test_lib :: ModerationRequest >"
                })
        })
        .expect("the backend must emit the core-to-binding conversion");
    let conversion = conversion.to_token_stream().to_string();
    assert!(
        conversion.contains(
            "input_value : val . input . as_ref () . and_then (| v | serde_wasm_bindgen :: to_value (v) . ok ())"
        ),
        "core-to-binding conversion must populate the lossless companion from the whole union: {conversion}"
    );
}
