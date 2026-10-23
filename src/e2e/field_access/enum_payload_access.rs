use crate::core::ir::{EnumDef, FieldDef};

use super::types::{FieldResolver, WireValueKind};

#[derive(Debug, Clone)]
pub struct EnumFieldMetadata {
    pub variant: String,
    pub field: String,
    pub definition: FieldDef,
    pub wire_key: String,
    pub wire_prefix: Vec<String>,
    pub next_type: Option<String>,
    pub kind: WireValueKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireAccessStep {
    Field(String),
    Index(usize),
}

struct WireField {
    keys: Vec<String>,
    next_type: Option<String>,
    kind: WireValueKind,
}

impl FieldResolver {
    pub fn enum_definition(&self, name: &str) -> Option<&EnumDef> {
        self.ir_enum_map.enum_definitions.get(name)
    }

    pub fn enum_field_metadata(&self, owner: &str, requested: &str) -> Option<EnumFieldMetadata> {
        let shapes = self.ir_enum_map.variant_shapes.get(owner)?;
        let mut candidates = shapes.iter().filter(|shape| !shape.is_tuple).flat_map(|shape| {
            shape.fields.iter().filter_map(move |(field, next)| {
                let wire = shape.python_wire_fields.get(field)?;
                (field == requested || wire == requested).then_some((shape, field, next, wire))
            })
        });
        let (shape, field, next_type, wire_key) = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        let definition = self
            .enum_definition(owner)?
            .variants
            .iter()
            .find(|variant| variant.name == shape.name)?
            .fields
            .iter()
            .find(|definition| definition.name == *field)?
            .clone();
        Some(EnumFieldMetadata {
            variant: shape.name.clone(),
            field: field.clone(),
            definition,
            wire_key: wire_key.clone(),
            wire_prefix: shape.python_wire_prefix.clone(),
            next_type: next_type.clone(),
            kind: shape.wire_kinds.get(field).copied().unwrap_or_default(),
        })
    }

    pub fn enum_payload_wire_prefix(&self, union_type: &str, variant: &str) -> Option<Vec<String>> {
        self.ir_enum_map
            .variant_shapes
            .get(union_type)?
            .iter()
            .find(|shape| shape.name == variant)
            .map(|shape| shape.python_wire_prefix.clone())
    }

    pub fn enum_payload_wire_steps(
        &self,
        union_type: &str,
        variant: &str,
        suffix: &str,
    ) -> Option<Vec<WireAccessStep>> {
        if suffix.is_empty() {
            return self.enum_payload_wire_prefix(union_type, variant).map(|_| Vec::new());
        }
        let mut owner = Some(self.union_variant_payload(union_type, variant)?.1.to_string());
        let mut steps = Vec::new();
        for (offset, segment) in suffix.split('.').enumerate() {
            if segment.is_empty() {
                return None;
            }
            let (field, indices) = segment.split_at(segment.find('[').unwrap_or(segment.len()));
            if !field.is_empty() {
                let resolved = self.wire_field(owner.as_deref()?, field)?;
                if !indices.is_empty() && resolved.kind != WireValueKind::Collection {
                    return None;
                }
                steps.extend(resolved.keys.into_iter().map(WireAccessStep::Field));
                owner = resolved.next_type;
            } else if offset != 0 || !self.union_variant_payload_is_collection(union_type, variant) {
                return None;
            }
            steps.extend(numeric_indices(indices)?.into_iter().map(WireAccessStep::Index));
        }
        Some(steps)
    }

    fn wire_field(&self, owner: &str, requested: &str) -> Option<WireField> {
        if self.ir_enum_map.variant_shapes.contains_key(owner) {
            let metadata = self.enum_field_metadata(owner, requested)?;
            let mut keys = metadata.wire_prefix;
            keys.push(metadata.wire_key);
            return Some(WireField {
                keys,
                next_type: metadata.next_type,
                kind: metadata.kind,
            });
        }
        let fields = self.ir_enum_map.struct_wire_fields.get(owner)?;
        let mut matches = fields
            .iter()
            .filter(|(field, wire)| field.as_str() == requested || wire.as_str() == requested);
        let (field, wire) = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let next_type = self
            .ir_enum_map
            .field_types
            .get(owner)
            .and_then(|fields| fields.get(field))
            .or_else(|| {
                self.ir_enum_map
                    .enum_field_types
                    .get(owner)
                    .and_then(|fields| fields.get(field))
            })
            .cloned();
        Some(WireField {
            keys: vec![wire.clone()],
            next_type,
            kind: self
                .ir_enum_map
                .struct_wire_kinds
                .get(owner)
                .and_then(|fields| fields.get(field))
                .copied()
                .unwrap_or_default(),
        })
    }
}

pub(super) fn numeric_indices(mut suffix: &str) -> Option<Vec<usize>> {
    let mut indices = Vec::new();
    while !suffix.is_empty() {
        let (index, rest) = suffix.strip_prefix('[')?.split_once(']')?;
        if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        indices.push(index.parse().ok()?);
        suffix = rest;
    }
    Some(indices)
}

#[cfg(test)]
mod tests;
