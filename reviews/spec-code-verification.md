# `SPEC_corrected.md` ↔ implementation verification

Status: the findings from the first pass are resolved in the code and the
specification. This document lists only what is still open.

Baseline: `cargo build` is clean; `cargo test` passes 248 tests.

## Open findings

### F6.3.1 [Major, deferred] Idle draw and power rounding are not modelled

- Spec: §6.3 states active power as `energy_usage × multiplier` and carries a
  TODO: a machine also draws its idle energy when it is not crafting, and a
  machine count is rounded up to whole machines for the power total. Neither
  rule is written yet.
- Code: `crates/hactorio-data/src/registry.rs:148-149` parses only
  `energy_usage_w`; the dump's idle energy is not carried, and
  `crates/hactorio-core/src/new_eval/process.rs:181` computes
  `power_w = machine.energy_usage_w * consumption`.
- Effect: `.power`, `.energy`, and `.:electric:` report active draw only, and a
  fractional machine count contributes fractionally. The numbers are not final.
- Disposition: deferred to a planned feature; the specification marks the hole
  so the current behaviour is not mistaken for the intended one.
