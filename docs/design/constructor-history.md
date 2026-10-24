# Preserve Swift constructor signatures

Record released signatures before adding fields to a transparent Rust DTO with `Default`.
`legacy_constructor_fields` preserves its existing meaning. Add `constructor_history` to record
additional released signatures in chronological order:

```rust
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(alef, alef(
    legacy_constructor_fields = ["count"],
    constructor_history = [["count", "enabled"]]
))]
pub struct Options {
    pub count: u32,
    pub enabled: bool,
    pub include_details: bool,
    pub escape_misc: bool,
    pub escape_marks: bool,
    pub include_labels: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            count: 7,
            enabled: true,
            include_details: true,
            escape_misc: false,
            escape_marks: false,
            include_labels: true,
        }
    }
}
```

Swift exposes exact public initializer overloads with one, two, and six parameters. Assigning
`Options.init(count:enabled:)` to a two-argument function value continues to work. Defaulted
parameters alone cannot preserve that function value's arity.

RustBridge retains the original one-argument initializer and exposes both two- and six-argument
`new_with_fields` factory overloads. The two-argument factory keeps the existing native
`__swift_bridge__$Options$new_with_fields` symbol and parameter list. The six-argument factory
uses the reserved internal symbol `__alef_constructor_history_1`, while its Swift name remains
`new_with_fields`. Appending another recorded signature keeps those symbol ordinals stable.
Old compiled calls therefore keep targeting the same native signature.

You can also use `constructor_history` without a legacy annotation:

```rust
#[cfg_attr(alef, alef(constructor_history = [["count"], ["count", "enabled"]]))]
```

In that case, RustBridge exposes exact initializer overloads for all three signatures. The
oldest recorded signature keeps the native `new` symbol; later signatures receive reserved
internal symbols. The current full initializer always remains available.

Record a zero-argument signature as the first entry, `constructor_history = [[], ["count"]]`,
when no legacy annotation is present. Every omitted field must have a resolved Rust default.
The generated public and RustBridge zero-argument initializers can then be used as `() -> DTO`
function values.

Use field names in their Rust declaration order. Every signature must extend the previous
recorded signature without removing or reordering fields. Duplicate signatures, unknown or
excluded fields, unresolved omitted-field defaults, and a history entry duplicating the current
full constructor fail validation. Alef emits only explicitly recorded signatures and the current
full signature. Keep recording each released full signature before extending the DTO again.

Omitted fields use their resolved Rust defaults, including nonzero numbers and distinct `true`
and `false` values. Changing an existing field's name, type, order, or default still needs its own
compatibility handling. Other language backends retain their existing constructor emission.

Rust callers constructing `VersionAnnotation` literals must add `constructor_history: vec![]`
or use `..Default::default()`. Old serialized IR remains readable, and an empty history is omitted
from serialized output.
