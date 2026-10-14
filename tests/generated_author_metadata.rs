use alef::backends::kotlin_android::KotlinAndroidBackend;
use alef::core::backend::Backend;
use alef::core::config::{Language, NewAlefConfig, ResolvedCrateConfig};
use alef::core::ir::ApiSurface;

fn config(authors: &[&str]) -> ResolvedCrateConfig {
    let authors = authors
        .iter()
        .map(|author| serde_json::to_string(author).expect("serialize author"))
        .collect::<Vec<_>>()
        .join(", ");
    let input = format!(
        r#"[workspace]
languages = ["r", "kotlin_android"]
[[crates]]
name = "example"
sources = ["src/lib.rs"]
[crates.r]
package_name = "example"
[crates.kotlin_android]
package = "dev.example"
[crates.package_metadata]
repository = "https://github.com/example/example"
license = "MIT"
authors = [{authors}]
"#
    );
    let input: NewAlefConfig = toml::from_str(&input).expect("parse config");
    input.resolve().expect("resolve config").remove(0)
}

fn r_authors(authors: &[&str]) -> String {
    let files = alef::scaffold::scaffold(&ApiSurface::default(), &config(authors), &[Language::R])
        .expect("generate R scaffold");
    files
        .iter()
        .find(|file| file.path.ends_with("DESCRIPTION"))
        .expect("DESCRIPTION output")
        .content
        .lines()
        .find(|line| line.starts_with("Authors@R:"))
        .expect("Authors@R field")
        .to_owned()
}

#[test]
fn should_preserve_all_r_authors_with_one_creator() {
    assert_eq!(
        r_authors(&["Ada Lovelace <ada@example.com>", "Grace Hopper <grace@example.com>"]),
        concat!(
            r#"Authors@R: c(person("Ada", "Lovelace", email = "ada@example.com", role = c("aut", "cre")), "#,
            r#"person("Grace", "Hopper", email = "grace@example.com", role = "aut"))"#
        )
    );
}

#[test]
fn should_preserve_single_r_author_output() {
    for (author, expected) in [
        (
            "Ada Lovelace <ada@example.com>",
            r#"Authors@R: person("Ada", "Lovelace", email = "ada@example.com", role = c("aut", "cre"))"#,
        ),
        (
            "O'Neil Research <oneil@example.com>",
            r#"Authors@R: person("O'Neil", "Research", email = "oneil@example.com", role = c("aut", "cre"))"#,
        ),
        (
            "Example Team",
            r#"Authors@R: person("Example Team", role = c("aut", "cre"))"#,
        ),
    ] {
        assert_eq!(r_authors(&[author]), expected);
    }
}

#[test]
fn should_parse_author_email_after_display_name_closing_bracket() {
    assert_eq!(
        r_authors(&["Team > Division <team@example.com>"]),
        concat!(
            r#"Authors@R: person("Team >", "Division", email = "team@example.com", "#,
            r#"role = c("aut", "cre"))"#
        )
    );
    for (input, expected) in [
        (
            "Team > Division <team@example.com>",
            ("Team > Division", "team@example.com"),
        ),
        ("Solo > Team", ("Solo > Team", "")),
        ("Team <team@example.com>", ("Team", "team@example.com")),
        ("<team@example.com>", ("", "team@example.com")),
        ("Team <missing", ("Team <missing", "")),
        ("Team > <missing", ("Team > <missing", "")),
        ("Team > <>", ("Team >", "")),
        ("<>", ("", "")),
        (
            "Équipe > Division <team@example.com>",
            ("Équipe > Division", "team@example.com"),
        ),
    ] {
        assert_eq!(alef::scaffold::parse_author(input), expected, "{input}");
    }
}

#[test]
fn should_trim_r_author_email_and_omit_whitespace_only_email() {
    assert_eq!(
        r_authors(&["Ada Lovelace < ada@example.com >"]),
        r#"Authors@R: person("Ada", "Lovelace", email = "ada@example.com", role = c("aut", "cre"))"#
    );
    assert_eq!(
        r_authors(&["Ada Lovelace < >"]),
        r#"Authors@R: person("Ada Lovelace", role = c("aut", "cre"))"#
    );
}

#[test]
fn should_escape_r_authors_and_preserve_names_without_family_or_email() {
    assert_eq!(
        r_authors(&[
            r#"A "quoted" Author <a@example.com>"#,
            "Solo <solo@example.com>",
            r"Team\Name"
        ]),
        concat!(
            r#"Authors@R: c(person("A \"quoted\"", "Author", email = "a@example.com", role = c("aut", "cre")), "#,
            r#"person("Solo", email = "solo@example.com", role = "aut"), "#,
            r#"person("Team\\Name", role = "aut"))"#
        )
    );
}

#[test]
fn should_emit_kotlin_author_literals_without_xml_entities_or_interpolation() {
    let files = KotlinAndroidBackend
        .generate_bindings(
            &ApiSurface::default(),
            &config(&[
                "O'Neil & Partners <oneil@example.com>",
                "A \"quote\" \\ $team\nName <two@example.com>",
            ]),
        )
        .expect("generate Kotlin Android");
    let gradle = &files
        .iter()
        .find(|file| file.path.ends_with("build.gradle.kts"))
        .expect("Gradle output")
        .content;
    let developers = gradle.split("developers {").nth(1).expect("developers block");
    let names = developers
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("name.set("))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            r#"name.set("O'Neil & Partners")"#,
            r#"name.set("A \"quote\" \\ \$team\nName")"#
        ]
    );
    assert_eq!(gradle.matches("email.set(").count(), 2);
}
