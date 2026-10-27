use alef::backends::magnus::MagnusBackend;
use alef::core::backend::Backend;
use alef::core::config::NewAlefConfig;
use alef::core::ir::{ApiSurface, EnumDef, EnumVariant, FieldDef, PrimitiveType, TypeRef};

fn generated_signature() -> String {
    let config: NewAlefConfig =
        toml::from_str("[workspace]\nlanguages = ['ruby']\n[[crates]]\nname = 'fixture'\nsources = ['src/lib.rs']\n[crates.ruby.stubs]\noutput = 'sig'\n")
            .unwrap();
    let fields = [
        ("label", TypeRef::String),
        ("count", TypeRef::Primitive(PrimitiveType::U64)),
        ("enabled", TypeRef::Primitive(PrimitiveType::Bool)),
    ]
    .into_iter()
    .map(|(name, ty)| FieldDef {
        name: name.into(),
        ty,
        optional: true,
        ..Default::default()
    })
    .collect();
    let api = ApiSurface {
        crate_name: "fixture".into(),
        enums: vec![EnumDef {
            name: "Message".into(),
            rust_path: "fixture::Message".into(),
            serde_tag: Some("kind".into()),
            variants: vec![EnumVariant {
                name: "Record".into(),
                fields,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    MagnusBackend
        .generate_type_stubs(&api, &config.resolve().unwrap().remove(0))
        .unwrap()
        .into_iter()
        .find(|file| file.path.extension().is_some_and(|extension| extension == "rbs"))
        .expect("generated RBS file")
        .content
}

#[test]
fn tagged_optional_constructor_matches_nullable_readers() {
    let signature = generated_signature();
    for expected in ["label: String?", "count: Integer?", "enabled: bool?"] {
        assert!(
            signature.contains(expected),
            "missing nullable parameter {expected}:\n{signature}"
        );
    }
    assert!(
        signature.contains("def self.new: (label: String?, count: Integer?, enabled: bool?)"),
        "{signature}"
    );
}

#[cfg(unix)]
#[test]
fn real_steep_accepts_nil_and_rejects_incorrect_constructor_types() {
    if std::process::Command::new("steep").arg("--version").output().is_err() {
        assert!(
            std::env::var_os("ALEF_REQUIRE_RUBY").is_none(),
            "Steep is required on Ruby CI"
        );
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    std::fs::create_dir(root.join("sig")).unwrap();
    std::fs::write(root.join("sig/fixture.rbs"), generated_signature()).unwrap();
    std::fs::write(
        root.join("Steepfile"),
        "target :fixture do\n  signature 'sig'\n  check 'check.rb'\nend\n",
    )
    .unwrap();
    let check = |source| {
        std::fs::write(root.join("check.rb"), source).unwrap();
        std::process::Command::new("steep")
            .args(["check", "--jobs=1"])
            .current_dir(root)
            .output()
            .unwrap()
    };
    let positive = check("Fixture::MessageRecord.new(label: nil, count: nil, enabled: nil)\n");
    assert!(
        positive.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&positive.stdout),
        String::from_utf8_lossy(&positive.stderr)
    );
    let negative = check("Fixture::MessageRecord.new(label: 123, count: nil, enabled: nil)\n");
    assert!(!negative.status.success(), "negative constructor control must fail");
    assert!(
        String::from_utf8_lossy(&negative.stdout).contains("Ruby::ArgumentTypeMismatch"),
        "{}",
        String::from_utf8_lossy(&negative.stdout)
    );
}
