use crate::core::config::{AdapterConfig, ResolvedCrateConfig};
use crate::core::ir::ErrorDef;
use quote::ToTokens;
use std::path::PathBuf;
use std::process::{Command, Output};

const LARGE_PAYLOAD_BYTES: usize = 1024;
const SMALL_PAYLOAD_BYTES: usize = 1;

enum ProbeLayout {
    Emitted,
    Inline,
}

fn wire_enum(unit_item: bool) -> syn::ItemEnum {
    let adapter: AdapterConfig = serde_json::from_value(serde_json::json!({
        "name": "events", "pattern": "streaming", "core_path": "events",
        "owner_type": "SampleEngine", "item_type": if unit_item { "()" } else { "SampleItem" },
        "error_type": "SampleError",
    }))
    .unwrap();
    let error: ErrorDef = serde_json::from_value(serde_json::json!({
        "name": "SampleError", "rust_path": "sample::SampleError", "variants": [], "doc": "",
    }))
    .unwrap();
    let mut generated = String::new();
    super::emit_transport(&mut generated, &adapter, &error, &ResolvedCrateConfig::default(), "");
    syn::parse_file(&generated)
        .unwrap()
        .items
        .into_iter()
        .find_map(|item| match item {
            syn::Item::Enum(item) => Some(item),
            _ => None,
        })
        .expect("the generated transport contains a wire enum")
}

fn remove_indirection(wire: &mut syn::ItemEnum) {
    for variant in &mut wire.variants {
        for field in &mut variant.fields {
            let syn::Type::Path(path) = &field.ty else { continue };
            let segment = path.path.segments.last().unwrap();
            if segment.ident != "Box" {
                continue;
            }
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                panic!("Box has one type argument");
            };
            let syn::GenericArgument::Type(inner) = arguments.args.first().unwrap() else {
                panic!("Box contains the payload type");
            };
            field.ty = inner.clone();
        }
    }
}

fn boxed_variant(wire: &syn::ItemEnum, name: &str) -> bool {
    wire.variants
        .iter()
        .find(|variant| variant.ident == name)
        .unwrap()
        .fields
        .iter()
        .any(|field| matches!(&field.ty, syn::Type::Path(path) if path.path.segments.last().unwrap().ident == "Box"))
}

fn source(item_size: usize, error_size: usize, unit_item: bool, layout: ProbeLayout) -> String {
    let mut wire = wire_enum(unit_item);
    if matches!(layout, ProbeLayout::Inline) {
        remove_indirection(&mut wire);
    }
    crate::backends::dart::template_env::render(
        "rust_streaming_layout_probe.rs.jinja",
        crate::alef_context! {
            declaration => wire.to_token_stream().to_string(),
            wire_name => wire.ident.to_string(),
            item_size => item_size,
            error_size => error_size,
            unit_item => unit_item,
            boxed_item => boxed_variant(&wire, "Item"),
            boxed_error => boxed_variant(&wire, "Error"),
        },
    )
}

struct CompiledProbe {
    directory: tempfile::TempDir,
    binary: PathBuf,
    output: Output,
}

fn compile(source: &str) -> CompiledProbe {
    let directory = tempfile::tempdir().unwrap();
    let compiler = which::which_in("clippy-driver", std::env::var_os("PATH"), directory.path()).unwrap();
    let input = directory.path().join("probe.rs");
    let binary = directory.path().join(if cfg!(windows) { "probe.exe" } else { "probe" });
    std::fs::write(&input, source).unwrap();
    let output = Command::new(compiler)
        .current_dir(directory.path())
        .args(["--edition=2024", "-D", "warnings"])
        .arg(input)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    CompiledProbe {
        directory,
        binary,
        output,
    }
}

#[test]
fn stream_transports_compile_for_large_and_small_payloads() {
    let shapes = [
        (LARGE_PAYLOAD_BYTES, SMALL_PAYLOAD_BYTES, false),
        (SMALL_PAYLOAD_BYTES, LARGE_PAYLOAD_BYTES, false),
        (SMALL_PAYLOAD_BYTES, SMALL_PAYLOAD_BYTES, false),
        (SMALL_PAYLOAD_BYTES, LARGE_PAYLOAD_BYTES, true),
    ];
    assert_eq!(shapes.len(), 4);
    for (item_size, error_size, unit_item) in shapes {
        let probe = compile(&source(item_size, error_size, unit_item, ProbeLayout::Emitted));
        assert!(
            probe.output.status.success(),
            "{}",
            String::from_utf8_lossy(&probe.output.stderr)
        );
        assert!(
            Command::new(&probe.binary)
                .current_dir(probe.directory.path())
                .status()
                .unwrap()
                .success()
        );
    }
}

#[test]
fn inline_stream_payload_negative_control_fires_the_large_variant_lint() {
    let shapes = [
        (LARGE_PAYLOAD_BYTES, SMALL_PAYLOAD_BYTES),
        (SMALL_PAYLOAD_BYTES, LARGE_PAYLOAD_BYTES),
    ];
    assert_eq!(shapes.len(), 2);
    for (item_size, error_size) in shapes {
        let probe = compile(&source(item_size, error_size, false, ProbeLayout::Inline));
        assert!(!probe.output.status.success());
        let stderr = String::from_utf8_lossy(&probe.output.stderr);
        assert!(stderr.contains("clippy::large_enum_variant"), "{stderr}");
    }
}
