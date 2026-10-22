use std::collections::{HashMap, HashSet};
use std::process::Command;

use crate::core::ir::{EnumDef, EnumVariant, FieldDef, TypeDef, TypeRef};
use crate::e2e::field_access::FieldResolver;
use crate::e2e::fixture::Assertion;

fn field(name: &str, ty: TypeRef) -> FieldDef {
    FieldDef {
        name: name.into(),
        ty,
        ..Default::default()
    }
}

fn schema() -> (Vec<TypeDef>, Vec<EnumDef>) {
    let types = vec![TypeDef {
        name: "Envelope".into(),
        fields: vec![field(
            "body",
            TypeRef::Optional(Box::new(TypeRef::Named("Payload".into()))),
        )],
        ..Default::default()
    }];
    let enums = vec![
        EnumDef {
            name: "Payload".into(),
            serde_untagged: true,
            variants: vec![EnumVariant {
                name: "Parts".into(),
                is_tuple: true,
                fields: vec![field("_0", TypeRef::Vec(Box::new(TypeRef::Named("Part".into()))))],
                ..Default::default()
            }],
            ..Default::default()
        },
        EnumDef {
            name: "Part".into(),
            serde_tag: Some("kind".into()),
            variants: vec![EnumVariant {
                name: "Text".into(),
                fields: vec![field("text", TypeRef::String)],
                ..Default::default()
            }],
            ..Default::default()
        },
    ];
    (types, enums)
}

fn resolver() -> FieldResolver {
    let (types, enums) = schema();
    FieldResolver::new(
        &HashMap::new(),
        &HashSet::from(["body".to_string()]),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
    )
    .with_ir_enum_map(FieldResolver::ir_enum_fields(&types, &enums), Some("Envelope".into()))
    .with_rust_enum_module("sample")
    .with_python_typeddict_map(FieldResolver::python_typeddict_fields(&types), Some("Envelope".into()))
}

fn assertion(kind: &str, path: &str, value: serde_json::Value) -> Assertion {
    Assertion {
        assertion_type: kind.into(),
        field: Some(path.into()),
        value: Some(value),
        ..Default::default()
    }
}

fn rust_assertion(assertion: &Assertion) -> String {
    let mut output = String::new();
    crate::e2e::codegen::rust::assertions::render_assertion(
        &mut output,
        assertion,
        "result",
        "sample",
        "sample",
        false,
        &[],
        &resolver(),
        false,
        false,
        false,
        false,
        false,
        None,
    );
    output
}

fn compile_and_run(assertions: &str) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("main.rs");
    let binary = dir.path().join(format!("check{}", std::env::consts::EXE_SUFFIX));
    let code = format!(
        r#"
mod sample {{
    pub struct Envelope {{ pub body: Option<Payload> }}
    pub enum Payload {{ Parts(Vec<Part>) }}
    pub enum Part {{ Text {{ text: String }} }}
}}
fn main() {{
    let result = sample::Envelope {{ body: Some(sample::Payload::Parts(vec![
        sample::Part::Text {{ text: "hello".into() }},
        sample::Part::Text {{ text: "world".into() }},
    ])) }};
    {assertions}
}}
"#
    );
    std::fs::write(&source, code).unwrap();
    let compile = Command::new("rustc")
        .arg("--edition=2024")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "generated assertions must compile:\n{assertions}\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    Command::new(binary).output().unwrap()
}

#[test]
fn rust_union_payload_assertions_compile_and_check_each_part() {
    let count = rust_assertion(&assertion("count_equals", "body.parts", serde_json::json!(2)));
    let first = rust_assertion(&assertion("equals", "body.parts[0].text", serde_json::json!("hello")));
    let second = rust_assertion(&assertion("equals", "body.parts[1].text", serde_json::json!("world")));
    assert!(compile_and_run(&format!("{count}{first}{second}")).status.success());
    let wrong = rust_assertion(&assertion("equals", "body.parts[0].text", serde_json::json!("wrong")));
    assert!(
        !compile_and_run(&wrong).status.success(),
        "the generated assertion must reject incorrect text"
    );
}

#[test]
fn python_union_payload_accessor_reads_serialized_part_dicts() {
    let accessor = resolver().accessor("body.parts[0].text", "python", "result");
    let program = format!(
        r#"
from types import SimpleNamespace
result = SimpleNamespace(body=SimpleNamespace(parts=[{{"kind": "Text", "text": "hello"}}]))
assert {accessor} == "hello"
result.body.parts[0]["text"] = "wrong"
assert {accessor} != "hello"
result.body = None
assert {accessor} is None
"#
    );
    let output = Command::new("python3").arg("-c").arg(program).output().unwrap();
    assert!(
        output.status.success(),
        "accessor: {accessor}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[derive(serde::Serialize)]
enum ExternalPart {
    #[serde(rename = "wireVariant")]
    Text {
        #[serde(rename = "wireText")]
        text_content: String,
    },
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "payload")]
enum AdjacentPart {
    Text {
        #[serde(rename = "wireText")]
        text_content: String,
    },
}

#[derive(serde::Serialize)]
#[serde(tag = "kind")]
enum InternalPart {
    Text {
        #[serde(rename = "wireText")]
        text_content: String,
    },
}

#[derive(serde::Serialize)]
#[serde(untagged)]
enum UntaggedPart {
    Text {
        #[serde(rename = "wireText")]
        text_content: String,
    },
}

fn python_runs(program: String) {
    let output = Command::new("python3").arg("-c").arg(&program).output().unwrap();
    assert!(
        output.status.success(),
        "{program}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn typescript_runs(program: String) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("check.ts");
    std::fs::write(&source, &program).unwrap();
    let compiled = Command::new("tsc")
        .args([
            "--strict",
            "--target",
            "ES2022",
            "--module",
            "commonjs",
            "--skipLibCheck",
        ])
        .arg(&source)
        .arg("--outDir")
        .arg(directory.path())
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{program}\n{}\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new("node")
        .arg(directory.path().join("check.js"))
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{program}\n{}",
        String::from_utf8_lossy(&executed.stderr)
    );
}

type SerializedPartCase = (Option<&'static str>, Option<&'static str>, bool, serde_json::Value);

fn serialized_parts() -> [SerializedPartCase; 4] {
    let text_content = "hello".to_string();
    [
        (
            None,
            None,
            false,
            serde_json::to_value(ExternalPart::Text {
                text_content: text_content.clone(),
            })
            .unwrap(),
        ),
        (
            Some("kind"),
            Some("payload"),
            false,
            serde_json::to_value(AdjacentPart::Text {
                text_content: text_content.clone(),
            })
            .unwrap(),
        ),
        (
            Some("kind"),
            None,
            false,
            serde_json::to_value(InternalPart::Text {
                text_content: text_content.clone(),
            })
            .unwrap(),
        ),
        (
            None,
            None,
            true,
            serde_json::to_value(UntaggedPart::Text { text_content }).unwrap(),
        ),
    ]
}

#[test]
fn python_union_payload_accessors_follow_real_serde_wire_shapes() {
    for (tag, content, untagged, payload) in serialized_parts() {
        let (types, mut enums) = schema();
        enums[1].serde_tag = tag.map(str::to_string);
        enums[1].serde_content = content.map(str::to_string);
        enums[1].serde_untagged = untagged;
        enums[1].variants[0].serde_rename = tag.is_none().then(|| "wireVariant".into());
        enums[1].variants[0].fields[0].name = "text_content".into();
        enums[1].variants[0].fields[0].serde_rename = Some("wireText".into());
        let resolver =
            resolver().with_ir_enum_map(FieldResolver::ir_enum_fields(&types, &enums), Some("Envelope".into()));
        let accessor = resolver.accessor("body.parts[0].text_content", "python", "result");
        let element = resolver.python_element_accessor("text_content", "body.parts", "item");
        for language in ["node", "wasm"] {
            let expression = resolver.accessor("body.parts[0].text_content", language, "result");
            typescript_runs(format!(
                r#"const result: {{body?: string | unknown[]}} = {{body:[{payload}]}};
const actual: string = {expression};
if (actual !== 'hello') throw new Error('wrong text');
result.body = [];
let rejected = false;
try {{ void ({expression}); }} catch {{ rejected = true; }}
if (!rejected) throw new Error('missing part accepted');"#
            ));
        }
        let quoted = serde_json::to_string(&payload.to_string()).unwrap();
        python_runs(format!(
            r#"import json
from types import SimpleNamespace
item = json.loads({quoted})
result = SimpleNamespace(body=SimpleNamespace(parts=[item]))
assert {accessor} == 'hello'
assert {element} == 'hello'
"#
        ));
    }
}

#[test]
fn python_native_variant_field_uses_the_collision_free_dict_getter() {
    let (types, mut enums) = schema();
    enums[1].variants[0].fields[0].name = "text_content".into();
    enums[1].variants[0].fields[0].serde_rename = Some("wireText".into());
    let resolver = resolver().with_ir_enum_map(FieldResolver::ir_enum_fields(&types, &enums), Some("Part".into()));
    let accessor = resolver.accessor("text_content", "python", "result");
    python_runs(format!(
        r#"from types import SimpleNamespace
result = SimpleNamespace(text_payload={{'wireText': 'hello'}})
assert {accessor} == 'hello'
"#
    ));
}

#[test]
fn node_and_wasm_bare_payloads_work_at_the_root_and_below_an_index() {
    let (mut types, enums) = schema();
    types[0].fields[0].ty = TypeRef::Vec(Box::new(TypeRef::Named("Payload".into())));
    for language in ["node", "wasm"] {
        let root = resolver().with_ir_enum_map(FieldResolver::ir_enum_fields(&types, &enums), Some("Payload".into()));
        let accessor = root.accessor("parts[0].text", language, "result");
        assert!(
            accessor.contains("Array.isArray") && accessor.contains("\"text\" in value"),
            "{accessor}"
        );
        let parts = root.accessor("parts", language, "result");
        typescript_runs(format!(
            r#"let result: string | unknown[] = [{{text:'hello'}}];
const parts: unknown[] = {parts};
if (parts.length !== 1) throw new Error('wrong count');
const text: string = {accessor};
if (text !== 'hello') throw new Error('wrong text');"#
        ));
        let indexed =
            resolver().with_ir_enum_map(FieldResolver::ir_enum_fields(&types, &enums), Some("Envelope".into()));
        let accessor = indexed.accessor("body[0].parts[1].text", language, "result");
        typescript_runs(format!(
            r#"const result = {{body:[ [{{text:'first'}}, {{text:'hello'}}] ]}};
const text: string = {accessor};
if (text !== 'hello') throw new Error('wrong indexed text');"#
        ));
    }
}

#[test]
fn rust_variant_lookup_uses_canonical_acronym_casing() {
    let (types, mut enums) = schema();
    enums[0].variants[0].name = "HTTPParts".into();
    let resolver = resolver().with_ir_enum_map(FieldResolver::ir_enum_fields(&types, &enums), Some("Envelope".into()));
    for variant in ["http_parts", "HTTPParts"] {
        let rendered = resolver.accessor(&format!("body.{variant}[0].text"), "rust", "result");
        assert!(rendered.contains("sample::Payload::HTTPParts(v)"), "{rendered}");
    }
}
