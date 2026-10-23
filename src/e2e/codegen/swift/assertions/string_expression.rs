use crate::e2e::field_access::FieldResolver;

pub(super) fn is_display_text(resolver: &FieldResolver, field: Option<&str>) -> bool {
    field.is_some_and(|field| {
        resolver.is_display_as_text(field)
            || resolver
                .ir_enum_type_name(field)
                .is_some_and(|name| resolver.is_text_projected_enum(&name))
    })
}

pub(super) struct StringExpression<'a> {
    pub field_expr: &'a str,
    pub field: Option<&'a str>,
    pub field_resolver: &'a FieldResolver,
    pub module_name: &'a str,
    pub field_is_display_as_text: bool,
    pub field_is_optional: bool,
    pub leaf_is_property_access: bool,
    pub leaf_getter_is_optional: bool,
    pub accessor_is_optional: bool,
    pub is_map_subscript: bool,
    pub bare_result_is_simple_option: bool,
    pub field_is_enum: bool,
}

impl StringExpression<'_> {
    pub(super) fn render(self) -> String {
        if self.is_projected() && !self.leaf_is_property_access {
            return self.bridge_string();
        }
        if self.field_is_display_as_text {
            return self.display();
        }
        if self.is_map_subscript {
            return format!("({} ?? \"\")", self.field_expr);
        }
        if self.leaf_is_property_access {
            return self.property();
        }
        self.bridge_string()
    }

    fn property(&self) -> String {
        let field_expr = self.field_expr;
        if self.field_is_enum && (self.field_is_optional || self.accessor_is_optional) {
            format!("(({field_expr})?.rawValue ?? \"\")")
        } else if self.field_is_enum {
            format!("{field_expr}.rawValue")
        } else if self.field_is_optional || self.accessor_is_optional || self.bare_result_is_simple_option {
            format!("({field_expr} ?? \"\")")
        } else {
            field_expr.to_string()
        }
    }

    fn is_projected(&self) -> bool {
        self.field
            .and_then(|field| self.field_resolver.ir_enum_type_name(field))
            .is_some_and(|name| self.field_resolver.is_text_projected_enum(&name))
    }

    fn bridge_string(&self) -> String {
        let field_expr = self.field_expr;
        if self.leaf_getter_is_optional || self.field_is_optional && !self.accessor_is_optional {
            format!("({field_expr}?.toString() ?? \"\")")
        } else if self.accessor_is_optional {
            format!("({field_expr}.toString() ?? \"\")")
        } else {
            format!("{field_expr}.toString()")
        }
    }

    fn display(&self) -> String {
        let Self {
            field_expr,
            field,
            field_resolver,
            module_name,
            leaf_getter_is_optional,
            accessor_is_optional,
            leaf_is_property_access,
            field_is_optional,
            ..
        } = *self;

        if !leaf_is_property_access
            && let Some(enum_name) = field.and_then(|field| field_resolver.ir_enum_type_name(field))
        {
            let json = if leaf_getter_is_optional || accessor_is_optional {
                format!("({field_expr}?.toString() ?? \"null\")")
            } else {
                format!("{field_expr}.toString()")
            };
            let enum_name = crate::backends::swift::naming::swift_source_ident(&enum_name);
            format!(
                "((try JSONDecoder().decode({module_name}.{enum_name}?.self, from: Data({json}.utf8)))?.text() ?? \"\")"
            )
        } else if field_is_optional || accessor_is_optional {
            format!("({field_expr}?.text() ?? \"\")")
        } else {
            format!("{field_expr}.text()")
        }
    }
}
