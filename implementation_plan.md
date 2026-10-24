# Implementation plan: facet value language

**Status:** implemented. The old parser/evaluator modules and their tests are
removed; open edge-case data questions are tracked in `awaiting_decisions.md`.

Source of truth: `2026-10-22_design_doc_2.md`. The operator matrix in
`facet_operator_table.md` is the arithmetic checklist.

## Goal

Replace the current single-monomial `Value` and `@`/belt-literal syntax with the
facet-based value model, explicit machine expressions, processes, rate/capacity
facets, and the new properties.

Non-goals for the first cutover: new UI polish, game-data extraction changes,
and new autocomplete scoring. The existing app shell, data registry, icon
pipeline, and persistence stay in place.

## Architecture

### Core value model (`crates/hactorio-core/src/facets.rs`, new)

- `Monomial { coef: f64, units: BTreeMap<BaseUnit, i64> }` — the item facet.
  `BaseUnit` stays the existing item/fluid/second/joule/craft enum.
- `FacetValue`:
  - `item: Option<Monomial>`
  - `recipe: Option<RecipeId>`
  - `machine: Option<MachineFacet>`
  - `process: Option<Process>`
  - `config: Option<Config>`
  - `belt: Option<BeltFacet>`
  - `rate: Option<RateFacet>`
  - `capacity: Option<CapacityFacet>`
- Invariants: process-only; machine carries its config; belt implies rate; no
  separate config with machine; recipe only with item/machine (or alone).
- `Cap = Full | Limit(u32)`, `RateFacet`/`CapacityFacet` with item/fluid medium,
  `BeltFacet { count, cap }`.
- Basic constructors and facet accessors first; operators follow in phase 2.
- Error type `FacetError` with stable variants for tests. Span stays in
  `EvalError` at the evaluator boundary.

### Parser/AST

- Tokens: remove `@` and belt abbreviations; add `With`, `Making`, `StackSize`
  signal/prototype, signals, modulo if needed by the final grammar.
- `Expr`:
  - `Literal(FacetValue)`
  - `Variable`
  - `Neg`
  - `Binary`
  - `Juxtapose`
  - `With`
  - `Making`
  - `In`
  - `StackSize`
  - `Property { value, selector }`
- Parse the `MachineExpr` production:
  `Primary ('with' ConfigExpr)* ('making' Operand)?`
- Juxtaposition dispatch: rate application, capacity application, config
  attach/merge, dimensionless scaling, then error (explicit `*` required for
  two unit-bearing values).
- `:stack-size:` takes one `Primary` and binds tighter than juxtaposition.

### Evaluator

- `Eval`/`LineOutput` can be replaced by `FacetValue` directly once the old
  `Eval::Scalar/Machine/Recipe` split is gone. Keep the `StatementOutput`
  wrapper and error spans.
- Implement operators in this order: `+`, unary `-`, `*`, `/`, `^`,
  juxtaposition, `with`, `making`, `in`, `:stack-size:`.
- Reuse `config.rs` machine/module arithmetic where possible. Rewrite
  `resolve_recipe` for product-unit selection, productivity gating, and the
  same-id tiebreaker.
- Process representation stores net, power/energy, recipes, and extent exactly
  as the design doc.

### Properties and display

- Implement `.rate`, `.capacity`, `.stack-size`, `.power`, `.energy`,
  `.electric`/`.:lightning:`, `.machines`, `.time`, `.crafts`, `.speed`,
  `.productivity`/`.prod`, `.pollution`, `.slots`, `.recipe`, unit selectors,
  and `.abs`.
- Update `format.rs`/`detail.rs` to read `FacetValue`; belt remains internal,
  with `.capacity`/`.rate` exposing it.

### Data/UI cutover

- Add the new registry fields needed for: inserter capacity research, beacon
  distribution/profile, machine `base_productivity`, belt speed, container
  capacity, equipment power/energy. Document any dump-schema changes in doc 1.
- Replace the core evaluator behind the existing UI API, then remove old
  `Eval`, `MachineValue`, `RecipeValue`, and `BeltTier` literal paths.

## Phases

1. **Facet value skeleton + unit tests.** `facets.rs`; constructors, canonical
   equality, invariant checks, `FacetSet` tests. No parser changes.
2. **Lexer/AST/parser.** New tokens and productions; remove `@`/belt
   abbreviations; parser tests for precedence, literal construction, and
   explicit multiplication.
3. **Arithmetic.** `+ - * / ^`, scalar exception, config integer scaling,
   isolated rate/capacity conversions, belt rules.
4. **Juxtaposition + config/process operators.** Attach/merge, `with`,
   `making`, `in`, auto-resolution, extent rules.
5. **Properties, formatting, details.** All selectors and output facets.
6. **UI cutover and test replacement.** Fixture expansion; delete old syntax
   tests; run §11 and §12 acceptance.
7. **Cleanup.** Remove dead old evaluator/belt-literal code and update docs.

## Acceptance

- `cargo test --workspace` passes.
- Every expression in §11 is exercised as success or marked error.
- Table-driven coverage for every facet pair/operator and §10 error.
- Explicit `*` cases: item×item, item×time, time×time error under
  juxtaposition.
- Config division, exponent domain, itemless belt stacking, and machine
  division tests from the design doc.

## Risks

- The old `Eval` split is used throughout `hactorio/src`; cutover will touch
  formatting, details, completion, and snapshots.
- `config.rs` currently assumes module-only machine config; beacon groups are a
  new shape.
- Data import must supply fields for inserters/beacons that may not exist in the
  current dump.

## Decisions

All initial design questions are resolved; see `awaiting_decisions.md`.
