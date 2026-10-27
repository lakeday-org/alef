---
type: Concept
title: Architecture
x-ai-rulez:
  kind: context
  id: architecture
  metadata:
    priority: high
---

# Alef Architecture

Pipeline: `extract → core → codegen → backends::<lang>`, in one root-flat crate `alef` (binary and library). Modules under `src/`:

- `extract/` — parses Rust source into IR (`ApiSurface`) with `syn`
- `core/` — IR types, config schema (`AlefConfig`), `Backend` trait, `AlefError`
- `codegen/` — shared generation utilities: type mapping, naming, generators, Jinja templates
- `backends/<lang>/` — one module per target language, each implements `Backend`
- `bin_cli/` — the binary's command dispatch: `args.rs`, `dispatch.rs::run(cli: Cli)`, per-command handlers
- `cli/` — library support: build cache, crate-filter resolution (`dispatch.rs::select_crates`), git helpers, breaking-change detection, version pinning
- `extensions/` (dylib/template loading), `adapters/` (framework adapters), `docs/`, `readme/`, `scaffold/`, `snippets/`, `e2e/`, `publish/`

- `src/codegen/naming.rs` is the naming-policy module (host identifiers, wire names, internal Rust names, ABI symbols).
- Create-once seeds (written only when absent, then user-owned) differ from marked outputs (alef header or `alef:hash:`, rewritten every generation); the marker scan in `src/core/hash/markers.rs` decides ownership, not the command. `GeneratedFile.generated_header` only controls the DO-NOT-EDIT header.
