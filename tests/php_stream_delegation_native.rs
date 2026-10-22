use alef::backends::php::PhpBackend;
use alef::core::backend::Backend;
use alef::core::config::{Language, NewAlefConfig};
use alef::core::ir::{ApiSurface, FunctionDef, MethodDef, ReceiverKind, TypeDef};
use quote::ToTokens;
use std::path::Path;
use std::process::{Command, Output};

fn generated_stream_source() -> String {
    let config: NewAlefConfig = toml::from_str(
        r#"
[workspace]
languages = ["php"]
[[crates]]
name = "sample-core"
sources = ["src/lib.rs"]
[crates.php]
extension_name = "sample"
[[crates.adapters]]
name = "events"
pattern = "streaming"
core_path = "events"
owner_type = "Engine"
item_type = "String"
"#,
    )
    .unwrap();
    let config = config.resolve().unwrap().remove(0);
    let api = ApiSurface {
        crate_name: "sample-core".into(),
        functions: vec![FunctionDef {
            name: "ping".into(),
            ..Default::default()
        }],
        types: vec![TypeDef {
            name: "Engine".into(),
            rust_path: "sample_core::Engine".into(),
            is_opaque: true,
            methods: vec![MethodDef {
                name: "events".into(),
                is_async: true,
                receiver: Some(ReceiverKind::Ref),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let files = PhpBackend.generate_bindings(&api, &config).unwrap();
    let library = files.iter().find(|f| f.path.ends_with("src/lib.rs")).unwrap();
    let methods = stream_methods(&library.content);
    let (_, handle) = alef::adapters::streaming::generate_body(&config.adapters[0], Language::Php, &config).unwrap();
    format!(
        "{}\n{}\n{}\n{}",
        fixture_source(),
        methods[0],
        methods[1],
        handle.unwrap()
    )
}

fn stream_methods(source: &str) -> Vec<String> {
    let mut methods = Vec::new();
    for item in syn::parse_file(source).unwrap().items {
        let syn::Item::Impl(block) = item else { continue };
        for item in block.items {
            let syn::ImplItem::Fn(mut method) = item else { continue };
            if method.sig.ident != "events" {
                continue;
            }
            let owner = if method.sig.receiver().is_some() {
                "Engine"
            } else {
                "Facade"
            };
            method.attrs.clear();
            methods.push(format!("impl {owner} {{ {} }}", method.into_token_stream()));
        }
    }
    assert_eq!(methods.len(), 2, "expected both the instance and facade methods");
    methods
}

#[test]
fn generated_stream_instance_and_facade_return_generator_zvals() {
    let source = generated_stream_source();
    let parsed = syn::parse_file(&source).unwrap();
    let mut count = 0;
    for item in parsed.items {
        let syn::Item::Impl(block) = item else { continue };
        for item in block.items {
            let syn::ImplItem::Fn(method) = item else { continue };
            if method.sig.ident == "events" || method.sig.ident == "into_generator" {
                let result = method.sig.output.into_token_stream().to_string();
                assert!(result.contains("ext_php_rs :: types :: Zval"), "{result}");
                assert!(!result.contains("Vec"), "{result}");
                count += 1;
            }
        }
    }
    assert_eq!(count, 3, "instance, facade and handle must all agree");
}

#[test]
#[ignore = "requires PHP development headers and the PHP executable"]
fn generated_stream_delegation_compiles_and_defers_chunk_errors() {
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path());
    let source = generated_stream_source();
    let old = source.replacen(
        "impl Facade { pub fn events (engine : & Engine) -> std :: result :: Result < ext_php_rs :: types :: Zval",
        "impl Facade { pub fn events (engine : & Engine) -> std :: result :: Result < Vec<String>",
        1,
    );
    assert_ne!(old, source, "negative control must mutate the actual facade signature");
    std::fs::write(fixture.path().join("src/lib.rs"), old).unwrap();
    let negative = cargo_task(fixture.path(), "check");
    let stderr = String::from_utf8_lossy(&negative.stderr);
    assert!(!negative.status.success() && stderr.contains("E0308"), "{stderr}");
    assert!(stderr.contains("Vec<String>") && stderr.contains("Zval"), "{stderr}");
    std::fs::write(fixture.path().join("src/lib.rs"), source).unwrap();
    let build = cargo_task(fixture.path(), "build");
    assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stderr));
    let library = fixture.path().join("target/debug").join(format!(
        "{}alef_php_stream_probe{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    assert!(library.is_file());
    let runtime = Command::new("php")
        .current_dir(fixture.path())
        .arg("-d")
        .arg(format!("extension={}", library.display()))
        .arg("-r")
        .arg(php_probe())
        .output()
        .unwrap();
    assert!(
        runtime.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&runtime.stdout),
        "2 chunks then deferred error\n"
    );
}

fn write_fixture(path: &Path) {
    std::fs::create_dir(path.join("src")).unwrap();
    std::fs::create_dir(path.join(".cargo")).unwrap();
    std::fs::write(
        path.join(".cargo/config.toml"),
        "[target.'cfg(target_os = \"macos\")']\nrustflags = [\"-C\", \"link-arg=-Wl,-undefined,dynamic_lookup\"]\n",
    )
    .unwrap();
    std::fs::write(
        path.join("Cargo.toml"),
        r#"[package]
name = "alef-php-stream-probe"
version = "0.0.0"
edition = "2024"
[lib]
crate-type = ["cdylib"]
[dependencies]
ext-php-rs = "=0.16.3"
futures-util = "0.3"
serde_json = "1"
anyhow = "1"
tokio = { version = "1", features = ["rt-multi-thread"] }
"#,
    )
    .unwrap();
    std::fs::write(path.join("Taskfile.yml"),
        "version: '3'\ntasks:\n  check:\n    cmds: [cargo check --offline]\n  build:\n    cmds: [cargo build --offline]\n").unwrap();
}

fn cargo_task(path: &Path, task: &str) -> Output {
    Command::new("task")
        .current_dir(path)
        .arg("--dir")
        .arg(path)
        .arg(task)
        .env("CARGO_TARGET_DIR", path.join("target"))
        .output()
        .unwrap()
}

fn fixture_source() -> &'static str {
    r#"
use ext_php_rs::prelude::*;
static WORKER_RUNTIME: std::sync::LazyLock<tokio::runtime::Runtime> =
    std::sync::LazyLock::new(|| tokio::runtime::Runtime::new().unwrap());
mod sample_core {
    pub struct Engine;
    impl Engine {
        pub async fn events(&self) -> Result<
            futures_util::stream::BoxStream<'static, Result<String, anyhow::Error>>, anyhow::Error> {
            use futures_util::StreamExt;
            Ok(futures_util::stream::iter([
                Ok("first".into()), Ok("second".into()), Err(anyhow::anyhow!("stream failed"))
            ]).boxed())
        }
    }
}
struct Engine { inner: std::sync::Arc<sample_core::Engine> }
struct Facade;
fn alef_php_err(error: impl std::fmt::Display) -> ext_php_rs::exception::PhpException {
    ext_php_rs::exception::PhpException::from_message(error.to_string())
}
#[php_function]
fn sample_stream() -> Result<ext_php_rs::types::Zval, ext_php_rs::exception::PhpException> {
    Facade::events(&Engine { inner: std::sync::Arc::new(sample_core::Engine) })
}
#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module.class::<EngineEventsHandle>().function(wrap_function!(sample_stream))
}
"#
}

fn php_probe() -> &'static str {
    r#"
$generator = sample_stream();
if (!($generator instanceof Generator)) { throw new Exception('not a generator'); }
$chunks = [];
$failed = false;
try { foreach ($generator as $chunk) { $chunks[] = json_decode($chunk, true); } }
catch (Throwable $error) {
    if ($error->getMessage() !== 'stream failed') { throw $error; }
    $failed = true;
}
if (!$failed || $chunks !== ['first', 'second']) { throw new Exception('stream semantics changed'); }
echo "2 chunks then deferred error\n";
"#
}
