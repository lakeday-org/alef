use super::FieldResolver;

impl FieldResolver {
    pub(crate) fn with_text_projected_enums(mut self, names: &[String]) -> Self {
        self.text_projected_enum_names = names.iter().cloned().collect();
        self
    }

    pub(crate) fn is_text_projected_enum(&self, name: &str) -> bool {
        self.text_projected_enum_names.contains(name)
    }

    pub(crate) fn text_projected_crossing(&self, path: &str) -> Option<String> {
        let (_, owner, _, _) = self.ir_tagged_union_split(self.resolve(path))?;
        self.is_text_projected_enum(&owner).then_some(owner)
    }
}
