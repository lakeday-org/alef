---
type: Playbook
title: Regen Audit
description: "Test an alef change against consumer repos: local binary, cache-key levers, focused checks, downstream smoke test, triage of each warning. Use when smoke-testing alef in a consumer or when a codegen fix seems to have no effect."
x-ai-rulez:
  kind: skill
  id: regen-audit
  metadata:
    license: MIT
    name: regen-audit
---

# Regen Audit

The consumer-side audit rules (read every warning, cache hits, `alef:hash:` churn, what `alef verify`
does not check) are in the served skill `skills/alef/SKILL.md`, "Regeneration Is an Audit". This skill
adds what only applies when the change is in alef.

## Use the local binary

- After every change under `src/`, run `cargo install --path . --force` from the alef root; `~/.cargo/bin/alef` shadows a crates.io install.
- Check `which alef` and `alef --version`. Do not `cargo install alef` from crates.io until the change is released.
- Or run without installing: `cargo run -q --manifest-path <polyrepo-root>/alef/Cargo.toml -- all --clean` from the consumer.

## Prove the generator ran

`compute_inputs_hash` (`src/core/hash.rs`) excludes `ALEF_REV`, and `strip_alef_version_pin` strips `alef_version` before hashing, so a version pin change does not change the cache key. The invalidation lever is `CODEGEN_FORMAT_VERSION` (`src/core/template_versions.rs`). Check `.alef/hashes/` and `.alef/<crate>/` timestamps when the file count looks low.

## Checks and downstream smoke test

- Iterate with `mise run format:check`, `mise run lint` and `mise run test:run -- <test-filter>`.
- For docs/template work, smoke test consumers with the local binary, then `git diff --check` there.
- Pick consumer repositories in the polyrepo root whose `alef.toml` exercises the changed surface; cover the relevant languages and features.
- Never discard a consumer's uncommitted regeneration state.

## Triage each finding

- Alef codegen/extraction bug (reproduces in alef's e2e tests, or `src/extract/` misses an item): fix in alef with a `CHANGELOG.md` `[Unreleased]` entry and a regression test.
- Alef-owned workflow/action bug: fix that repository, retag only its documented action tags.
- Consumer config gap (missing `exclude_types`, wrong `[crates.<lang>]` override): fix the consumer's `alef.toml`, not alef.
- Do not carry a warning across releases because "it has always been there".
