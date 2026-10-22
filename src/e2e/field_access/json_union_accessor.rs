use super::optional_renderers::{TypescriptMapAccess, render_typescript_with_optionals};
use super::renderers::quoted_key_literal;
use super::types::{FieldResolver, PathSegment, WireValueKind};

impl FieldResolver {
    pub(super) fn typescript_json_union_accessor(
        &self,
        segments: &[PathSegment],
        result: &str,
        map_access: TypescriptMapAccess,
    ) -> Option<String> {
        let first = self.first_bare_variant(segments)?;
        let mut expression =
            render_typescript_with_optionals(&segments[..first], result, &self.optional_fields, map_access);
        let steps = self.python_access_steps(segments);
        let mut key = String::new();
        for (index, segment) in segments.iter().enumerate() {
            super::optional_renderers::push_key_field_name(&mut key, segment);
            if index >= first {
                let step = steps.get(&key);
                if let Some(step) = step {
                    for key in &step.json_keys {
                        expression = json_read(&expression, key, "member");
                    }
                    expression = validate_kind(expression, step.kind);
                }
                expression = render_json_segment(expression, segment, map_access);
            }
            super::optional_renderers::push_key_index_suffix(&mut key, segment);
        }
        Some(expression)
    }
}

fn validate_kind(expression: String, kind: WireValueKind) -> String {
    let guard = match kind {
        WireValueKind::Unknown => return expression,
        WireValueKind::String => "string",
        WireValueKind::Collection => "array",
    };
    crate::e2e::template_env::render(
        "typescript/json_guard.ts.jinja",
        minijinja::context! {
            value => expression, guard,
        },
    )
    .trim_end()
    .to_string()
}

fn render_json_segment(expression: String, segment: &PathSegment, map_access: TypescriptMapAccess) -> String {
    match segment {
        PathSegment::ArrayField { index, .. } => {
            let array = validate_kind(expression, WireValueKind::Collection);
            format!("{array}[{index}]")
        }
        PathSegment::MapAccess { key, .. } => json_read(
            &expression,
            key,
            match map_access {
                TypescriptMapAccess::Index => "member",
                TypescriptMapAccess::MapGet => "map",
            },
        ),
        PathSegment::Length => crate::e2e::template_env::render(
            "typescript/json_guard.ts.jinja",
            minijinja::context! {
                value => expression, guard => "length",
            },
        )
        .trim_end()
        .to_string(),
        PathSegment::Field(_) => expression,
    }
}

fn json_read(expression: &str, key: &str, kind: &str) -> String {
    crate::e2e::template_env::render(
        "typescript/json_member.ts.jinja",
        minijinja::context! {
            value => expression, key => quoted_key_literal(key), kind,
        },
    )
    .trim_end()
    .to_string()
}
