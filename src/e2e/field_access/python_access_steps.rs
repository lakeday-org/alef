use std::collections::HashMap;

use super::optional_renderers::{push_key_field_name, push_key_index_suffix};
use super::parse::segment_name;
use super::python_renderer::PythonAccessStep;
use super::types::{FieldResolver, PathSegment};

impl FieldResolver {
    pub(super) fn python_element_access_steps(&self, element: &str, array: &str) -> HashMap<String, PythonAccessStep> {
        let indexed = format!("{array}[0]");
        let prefix = format!("{}.", super::parse::normalize_numeric_indices(&indexed));
        let full = super::parse::parse_path(&format!("{indexed}.{element}"));
        self.python_access_steps(&full)
            .into_iter()
            .filter_map(|(key, step)| key.strip_prefix(&prefix).map(|relative| (relative.to_string(), step)))
            .collect()
    }

    pub(super) fn python_access_steps(&self, segments: &[PathSegment]) -> HashMap<String, PythonAccessStep> {
        let crossings: HashMap<_, _> = self
            .variant_crossings(segments)
            .into_iter()
            .map(|crossing| (crossing.key.clone(), crossing))
            .collect();
        let mut steps = HashMap::new();
        let mut cursor = self.ir_enum_map.root_type.clone();
        let mut key = String::new();
        for segment in segments {
            let Some(name) = segment_name(segment) else { continue };
            push_key_field_name(&mut key, segment);
            if let Some(crossing) = crossings.get(&key) {
                steps.insert(key.clone(), crossing.python_step());
                cursor = crossing.output_type.clone();
            } else if let Some(owner) = cursor.take() {
                if let Some(wire) = self
                    .ir_enum_map
                    .struct_wire_fields
                    .get(&owner)
                    .and_then(|fields| fields.get(name))
                {
                    steps.insert(
                        key.clone(),
                        PythonAccessStep {
                            native_suffix: None,
                            json_keys: vec![wire.clone()],
                            kind: self
                                .ir_enum_map
                                .struct_wire_kinds
                                .get(&owner)
                                .and_then(|fields| fields.get(name))
                                .copied()
                                .unwrap_or_default(),
                            starts_json: false,
                        },
                    );
                }
                cursor = self.python_next_owner(segment, &owner, name);
            }
            push_key_index_suffix(&mut key, segment);
        }
        steps
    }
    fn python_next_owner(&self, segment: &PathSegment, owner: &str, name: &str) -> Option<String> {
        let map_value = matches!(segment, PathSegment::MapAccess { .. })
            .then(|| {
                self.python_map_value_edges
                    .get(owner)
                    .and_then(|fields| fields.get(name))
            })
            .flatten();
        map_value
            .or_else(|| {
                self.ir_enum_map
                    .field_types
                    .get(owner)
                    .and_then(|fields| fields.get(name))
                    .or_else(|| {
                        self.ir_enum_map
                            .enum_field_types
                            .get(owner)
                            .and_then(|fields| fields.get(name))
                    })
            })
            .cloned()
    }
}

pub(super) fn json_keys<'a>(keys: impl IntoIterator<Item = &'a str>) -> String {
    keys.into_iter()
        .map(|key| {
            let quoted = serde_json::to_string(key).expect("JSON key is serializable");
            format!("[{quoted}]")
        })
        .collect()
}
