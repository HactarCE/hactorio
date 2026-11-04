# `SPEC.md` ↔ implementation verification

Scope: every normative claim in `SPEC.md` (1503 lines) checked against
`crates/hactorio-core/src/{syntax,text,settings}.rs`, `crates/hactorio-core/src/new_eval/*`,
`crates/hactorio/src/{evaluator,document,glyphs}.rs`, `crates/hactorio/src/app/**`, and
`crates/hactorio-data/src/*`.

`EXPRESSION_LANGUAGE.md` is referenced by `SPEC.md:5-6` ("`EXPRESSION_LANGUAGE.md`
describes the implementation being replaced and is not normative") but is **not present**
in the repository (`ls EXPRESSION_LANGUAGE.md` → no such file), so it could not be
consulted and is not treated as evidence anywhere below.

Outcomes per claim: **MATCHES**, **DIVERGES** (code does something different; both sides
quoted), **NOT IMPLEMENTED** (no code path exists).

## Baseline

- `cargo build` — clean, no warnings (`Finished dev profile … in 4.13s`).
- `cargo test` — 232 tests, 0 failed: `hactorio` 106, `hactorio-core` 98,
  `hactorio-data` 3 + 5 (`tests/icons.rs`) + 20 (`tests/registry.rs`).
- Behaviour was exercised with a throwaway crate at `/tmp/specprobe` (path deps on
  `hactorio-core`/`hactorio-data`, a hand-written `--dump-data`-shaped JSON registry
  containing foundry/crusher/recycler/beacon/modules/belts/inserter/chest/tank/pump/
  barrels/lamp/accumulator and the recipes the examples need). Nothing under `crates/`
  was modified. Every "Run:" line below is literal probe output; `:id:` in probe output is
  `to_plaintext` of the rendered glyph string.

## Summary by spec section

Counts are the number of distinct claims I examined in that section; the matched claims
are listed in the section body (a bullet sometimes covers two closely related claims).
`DIVERGES`/`NOT IMPLEMENTED` counts equal the number of findings below, each of which is
backed by a run or a code path.

| Section | MATCHES | DIVERGES | NOT IMPL | Finding IDs |
|---|---|---|---|---|
| §2.1 Document structure | 6 | 0 | 0 | — |
| §2.2 Definitions | 5 | 2 | 0 | F2.2.1, F2.2.2 |
| §2.3 Environment | 1 | 0 | 0 | — |
| §2.4 Evaluation order | 1 | 0 | 0 | — |
| §2.5 Errors | 4 | 0 | 0 | — |
| §2.6 Loops | 5 | 0 | 0 | — |
| §2.7 Cancellation | 4 | 0 | 0 | — |
| §3.1 Facets | 7 | 0 | 0 | — |
| §3.2 Quality domains | 4 | 0 | 0 | — |
| §3.3 Quality operand | 9 | 2 | 0 | F3.3.1, F3.3.2 |
| §3.4 Coexistence | 3 | 1 | 0 | F3.4.1 |
| §3.5 Identity and equality | 4 | 0 | 0 | — |
| §4.1 Juxtaposition chains | 5 | 0 | 0 | — |
| §4.2 Quality values | 5 | 0 | 0 | — |
| §4.3 Precedence | 11 | 2 | 0 | F4.3.1, F4.3.2 |
| §4.4 Tokens | 13 | 2 | 0 | F4.4.1, F4.4.2 |
| §4.5 Qualified prototype tokens | 4 | 0 | 0 | — |
| §4.6 Quality names and aliases | 4 | 0 | 0 | — |
| §5.1 Reference facets | 6 | 1 | 0 | F5.1.1 |
| §5.2 Juxtaposition | 8 | 0 | 0 | — |
| §5.3 Quantity arithmetic | 16 | 3 | 0 | F5.3.1, F5.3.2, F5.3.3 |
| §5.4 Machine and process operators | 14 | 3 | 0 | F5.4.1, F5.4.2, F5.4.3 |
| §5.5 Config arithmetic | 2 | 1 | 0 | F5.5.1 |
| §6.1 Stacks and stack limits | 9 | 0 | 0 | — |
| §6.2 Config | 5 | 1 | 0 | F6.2.1 |
| §6.3 Machine | 8 | 1 | 0 | F6.3.1 |
| §6.4 Process | 10 | 0 | 0 | — |
| §6.5 Resource and recipe references | 2 | 0 | 0 | — |
| §6.6 Selectors | 17 | 1 | 2 | F6.6.1, F6.6.2, F6.6.3 |
| §7.1 Canonical facet order | 4 | 2 | 0 | F7.1.1, F7.1.2 |
| §7.2 Numbers | 7 | 0 | 0 | — |
| §7.3 Metric prefixes | 4 | 0 | 0 | — |
| §7.4 Time units | 3 | 0 | 0 | — |
| §7.5 Round-trippable renderings | 9 | 1 | 0 | F7.5.1 |
| §7.6 Stack limits | 3 | 0 | 0 | — |
| §7.7 Processes | 6 | 1 | 0 | F7.7.1 |
| §7.8 Settings that affect display | 3 | 0 | 2 | F7.8.1 |
| §8 Diagnostics | 6 | 1 | 0 | F8.1 |
| Appendix A | 16 | 0 | 0 | (F5.5.1 is the A.2 `C,scalar` row) |
| Appendix B | 3 | 0 | 1 | F6.6.1 (B.2); FB.1 is a spec-internal error, not a code divergence |

No **Blocker** was found: the spec is implementable and the code compiles, evaluates, and
passes its own suite. Findings that make a spec example unreachable or a stated rule false
are marked **Major**.

---

## §2 Program model

### Matched (§2.1–§2.7)

- One statement per line; a newline terminates (`eval.rs:76-80` splits `document.lines()`,
  `parse.rs:233` parses exactly one line). No continuation: `expect_end` rejects trailing
  tokens (`parse.rs:337-345`).
- Statement kinds are exactly the table's five (`parse.rs:201-210`, `Line`).
- `{` must end the line (`parse.rs:255-262`), `}` must be alone (`parse.rs:249-254`).
- Loop bodies nest: `find_loop_end` counts depth (`eval.rs:222-236`).
- Blank vs comment distinguished (`parse.rs:234-241`); `#` only starts a comment outside a
  token because `lex` consumes a whole `:…:` before looking at `#` (`lex.rs:211-213`,
  `239-247`).
- `ASSIGNOP` set is exactly `= += -= *= /=` (`lex.rs:134-138`, `parse.rs:263-269`).
- Compound assignment reads the binding and applies the binary op (`eval.rs:265-272`).
- Resource-row assignment: only `Selector::Resource` is accepted, only on a `Process`, with
  `=` replacing the row's total and the stored `net` being the difference
  (`eval.rs:929-1015`).
- Unspecified quality in a selector means normal (`eval.rs:946-950`).
- `=` is a statement: `1 = 2` and `(q = 1)` are parse errors; `a = b = 1` is a parse error
  (`parse.rs:233-300`; Run: `a = b = 1` → ``ERROR unexpected `=` ``).
- Flat global namespace, no scopes (`eval.rs:86`).
- Statements in order; loops re-run the body against the same `env` (`eval.rs:203-205`).
- Errors never stop evaluation: an error attaches to the line and the run continues
  (`eval.rs:136-215`); a failing body statement does not stop the iteration.
- Poisoning: `self.env.insert(place.name.clone(), result.clone())` stores the error
  (`eval.rs:274`), `lookup` re-raises it (`eval.rs:243-249`), a later success overwrites it.
- `loop N {`: count evaluated once before the body, must be a non-negative integer, `0`
  runs nothing (`eval.rs:171-211`, `iterations` at `eval.rs:874-889`). Run: `loop 0 {` +
  body + `}` leaves the body `(blank)`.
- A loop produces no value: header and `}` get `Output::None` (`eval.rs:201-202`).
- Cancellation is checked at every statement boundary including each loop iteration
  (`eval.rs:139-141`; the body is re-entered through `run` per iteration, `eval.rs:203-205`);
  a cancelled run returns empty `outputs`/`hashes`/`variables` (`eval.rs:96-102`).
- The worker cancels a superseded request and discards its result (`evaluator.rs:36-76`).

### F2.2.1 [Minor] A non-assignable selector is an evaluation error, not a parse error

- Spec `SPEC.md:109-110`: "The selector must be one the specification marks assignable;
  other selectors, chained selectors, and non-variable places are **parse errors**."
- Code: `parse.rs:542-585` (`try_place`) accepts *any* known selector, and
  `eval.rs:258-263` rejects it at run time.
- Run: `p = 1 :foundry: making (:normal: :iron-plate:)` then `p.machines = 1` →
  ``ERROR `machines` is not assignable; only resources are`` on line 1, with the whole
  line as the span (an `Output::Error`, not a `ParseError`).
- Minimal repro: `p.machines = 1`.

### F2.2.2 [Major] A prototype id *is* accepted as a definition target

- Spec `SPEC.md:117`: "A prototype id is not a variable name."
- Code: `try_place` (`parse.rs:542-585`) only rejects `is_reserved_word`
  (`lex.rs:405-409`), which knows nothing about the registry.
- Run:
  ```
  foundry = 1
  foundry + 1
  ```
  → line 0 evaluates to `1`, line 1 to `2`. Same for `beacon = 2` (then bare `beacon` reads
  the variable). The glyph token `:foundry:` still resolves to the prototype, so the two
  namespaces silently diverge.
- Minimal repro: `foundry = 1`.

---

## §3 Value model

### Matched (§3.1–§3.5)

- `MultiValue` has exactly the seven facets of the §3.1 table (`value.rs:1239-1248`):
  `quantity`, `stack_rate`, `stack_capacity`, `machine`, `config`, `resource`, `recipe`.
- `quantity` payload is `Amount` (number or `BTreeMap<Resource, f64>`) plus
  `per: Option<Resource>` and `i32` exponents for `second`/`joule`/`craft`
  (`value.rs:159-167`, `235-241`).
- `Stacks` is `BTreeMap<StackLimit, f64>` or `Fluid(f64)` (`value.rs:718-721`);
  `StackLimit` is `Full | Limit(u32)` (`value.rs:691-696`).
- `machine` = prototype + `Config` + `Option<f64>` count (`value.rs:976-981`); `config` =
  sorted module list + beacon-group map (`value.rs:845-849`, `BeaconGroup` 837-842).
- No `belt` facet (D42): a belt token sets `quantity`, `stack_rate`, `stack_capacity`
  (`eval.rs:382-385`).
- Concrete quality in quantity terms, `Option<Quality>` in reference facets: `Item` holds a
  `Quality` (`value.rs:121-125`), `ResourceRef.quality: Option<Quality>` (`value.rs:676-680`),
  `RecipeRef.quality: Option<Quality>` (`value.rs:684-687`). `:iron-plate:` produces
  `1 iron-plate/normal` plus an unspecified-quality reference (`eval.rs:361-377`).
- `Value` is `Multi(MultiValue) | Process(Process) | Quality(Quality)` (`value.rs:1609-1613`).
- A machine with a recipe is a `Process` with `Extent::Unscaled`, not a fourth kind
  (`value.rs:1035-1047`, `eval.rs:1255-1265`).
- No `f64` map keys: keys are `Resource` (id+quality), `StackLimit`, `(EntryId, Quality)`,
  `BeaconGroup`, `RowKey` (`value.rs:128-148`, `718`, `845-849`, `1049-1057`).
- Structural equality via `derive(PartialEq)` throughout; zero terms removed by
  `Amount::canonicalize` (`value.rs:177-190`), `Stacks::canonicalize` (`value.rs:728-737`),
  `Process::add` (`value.rs:1097-1134`).
- Approximate zero is canonicalization/display only: `is_zero` (`value.rs:15-17`) is used in
  `canonicalize`/formatting, never in `PartialEq`.

### F3.3.1 [Major] The quality operand accepts a counted beacon, promoting all of them

- Spec `SPEC.md:278-284`: "one beacon group with **beacon count exactly one** and an empty
  module list has that beacon's quality set … **Both counts must be one** … Allowing the
  counted form would make the operator's meaning depend on which side of the quality the
  count was written."
- Code `value.rs:949-961`:
  ```rust
  if self.beacons.len() == 1 && self.modules.is_empty() {
      let (group, count) = self.beacons.iter().next()?;
      let group = BeaconGroup { quality, ..group.clone() };
      return Some(Self { modules: Modules::new(),
                         beacons: BTreeMap::from([(group, *count)]) });
  }
  ```
  `count` is carried through unchecked.
- Run: `q = :rare:` then `q 2 :beacon:` → `2:rare::beacon:` (both beacons promoted). Spec
  requires an error. The module branch *does* check `count == 1` (`value.rs:963-964`), so
  `q 2 :speed-module-2:` correctly errors.

### F3.3.2 [Major] The quality operand promotes a beacon group that already holds modules

- Spec `SPEC.md:278-280`: "one beacon group with beacon count exactly one and an **empty
  module list**"; D9 (`SPEC.md:1449`): "a single beacon of count one with **no module
  list**, whose beacon is promoted and whose modules are untouched".
- Code: the beacon branch at `value.rs:950` only checks the *config's* top-level
  `self.modules.is_empty()`; `group.modules` is never examined.
- Run: `q = :rare:` then `q (:beacon: 2 :speed-module-2:)` →
  `:rare::beacon::speed-module-2::speed-module-2:` (beacon promoted, modules kept). Spec
  requires an error.
- **Spec is self-contradictory here**: `SPEC.md:431` gives the opposite example —
  "`q (:beacon: :speed-module-1:)     # a rare beacon with a common module`". Code follows
  the §4.2 example and violates §3.3/D9. One of the two must change.

### F3.4.1 [Nit] §3.4's "MVP implements the first and third variants" is stale

- Spec `SPEC.md:331-332`: "The MVP implements the first and third variants. `Process`
  arrives with `making`, because nothing can construct one before then."
- Code implements all three variants and constructs `Process` (`value.rs:1609-1613`,
  `eval.rs:1182-1272`). The same stale claim is repeated in the module doc comments
  (`value.rs:3-6`, `new_eval/mod.rs:1-7`), which say the stack/machine/config facets and the
  process "arrive with their increments" although all are implemented.

---

## §4 Grammar

### Precedence, level by level (§4.3 table, `SPEC.md:446-464`)

| Spec level | Form | Code | Verdict |
|---|---|---|---|
| 1 | prefix `:stacked-belt:` `:stack-size:` `:recycling:` | `parse_operand` → `parse_primary` (`parse.rs:443-464`) | MATCHES |
| 2 | juxtaposition, right | one n-ary `Juxtapose` (`parse.rs:409-418`), folded right-to-left (`eval.rs:344-350`) | MATCHES |
| 3 | `.property`, postfix | dot loop before the infix check (`parse.rs:354-358`) | MATCHES |
| 4 | `.` with a resource operand, left | same loop, chained | MATCHES |
| 5 | quality filters `< > <= >= == !=`, left | `BP_COMPARE = 30`, `lbp + 1` (`parse.rs:284`, `379`) | MATCHES |
| 6 | `^`, right | `BP_POW = 60`, `right_associative` (`parse.rs:291`, `379`) | MATCHES |
| 7 | unary `-`, prefix | `BP_NEG = 57` (`parse.rs:289`, `445-448`) | MATCHES |
| 8 | `/`, left | `BP_DIV = 55` (`parse.rs:290`) | MATCHES |
| 9 | `*`, left | `BP_MUL = 50` (`parse.rs:287`) | MATCHES |
| 10 | `-`, left | `BP_SUB = 45` (`parse.rs:286`) | MATCHES |
| 11 | `+`, left | `BP_ADD = 40` (`parse.rs:285`) | MATCHES |
| 12 | `making`, left | folded inside `parse_chain` (`parse.rs:420-429`), i.e. tighter than `^` | **DIVERGES** (F4.3.1) |
| 13 | `in` / `into` / `from`, left | `BP_IN = 10` (`parse.rs:283`), RHS restricted (`parse.rs:367`) | **DIVERGES** (F4.3.2) |

`=` is not an operator (`parse.rs:233-300`) — MATCHES. Other matched claims: `/` tighter
than `*` (Run `3 * 1 / 2` → `1.5`), `-` tighter than `+` (Run `1 - 2 + 3` → `2`), unary `-`
between `^` and `/` (Run `-2 ^ 2` → `-4`; `-2 * 3` → `-6`), `^` right-assoc (Run
`2 ^ 3 ^ 2` → `512`), dot looser than juxtaposition (Run `:transport-belt: .rate`),
juxtaposition tighter than `making` (Run `:foundry: 2 :speed-module-2: making …` builds the
configured machine first), quality filters looser than `.` (Run
`(1:iron-plate: + 2:rare::iron-plate:).:normal::iron-plate:` → `1:iron-plate:`), and
juxtaposition right-assoc with the §4.1 cost accepted (Run
`2 :speed-module-2: 3 :efficiency-module-1:` → 2 speed + **6** efficiency modules, exactly
as `SPEC.md:372-375` warns).

### F4.3.1 [Major] `making` binds tighter than every infix operator, and cannot chain

- Spec `SPEC.md:463`: "| 12 | `making` | left |", i.e. looser than `+` (11) and tighter
  only than `in`/`into`/`from` (13).
- Code: `parse_chain` consumes `making` itself (`parse.rs:420-429`) and `parse_bp` calls
  `parse_chain` before examining any infix token (`parse.rs:351-383`), so `making` is
  effectively tighter than `^`.
- Run: `:foundry: making :iron-plate: * 2` →
  `2 · -2.5:iron-ore:/s · 3.75:iron-plate:/s` — the *process* is scaled. Under the spec
  table `*` binds tighter, so the expression is `:foundry: making (:iron-plate: * 2)`,
  whose right operand has lost its `recipe` facet (scaling drops references, D28) and must
  be an error.
- Chaining also differs: the spec's left-assoc column implies `a making b making c` parses;
  Run `:foundry: making :iron-plate: making :copper-plate:` →
  ``ERROR unexpected `making` ``.
- Minimal repro: `:foundry: making :iron-plate: * 2`.

### F4.3.2 [Minor] `in`/`into`/`from` take a restricted right operand

- Spec `SPEC.md:464`: level 13, ordinary left-associative binary operators.
- Code: the RHS goes through `parse_machine_expr` (`parse.rs:367`, `434-441`), which accepts
  a juxtaposition chain plus dots and nothing else, so no arithmetic can appear on the right
  without parentheses.
- Run: `1 :iron-plate: into 2 * :foundry:` →
  ``ERROR `into` needs a process or machine facet (right: quantity)`` — the code parsed
  `(1 :iron-plate: into 2) * :foundry:`. The spec's table gives
  `1 :iron-plate: into (2 * :foundry:)`, a two-machine sizing.
- Minimal repro: `1 :iron-plate: into 2 * :foundry:`.

### Tokens (§4.4)

Matched: decimal literals with optional fraction (`12`, `12.5`, `.5` — `lex.rs:411-441`);
trailing dot is an error (`3.` → "a number cannot end with `.`"); no exponent notation
(`1e-12` → error); scale suffixes `k`/`M`/`G`/`T` fold into the number (`1k` → `1000`,
`1M` → `1e6`); case sensitivity (`1m` → `60s`, minutes); the complete unit-keyword set
`second/s/minute/m/hour/h/joule/J/kJ/MJ/GJ/TJ/watt/W/kW/MW/GW/TW/craft` (`lex.rs:87-105`);
adjacency (`3s` → number+second, `3kJ` → number+kJ, `3x` → error, `3 x` → number+identifier
(`ERROR x is not defined` only at evaluation)); identifiers `[A-Za-z_][A-Za-z0-9_]*`
(`lex.rs:458-469`); prototype tokens resolve through the registry or glyph, empty/unterminated
tokens error (`lex.rs:239-247`, `471-485`); signal set and glyph table (`lex.rs:17-45`,
`249-259`); prefix signals vs dot-only signals (`lex.rs:78-86`, `parse.rs:441-471`,
`parse.rs:499-524`); quality tokens recognised before prototypes (`lex.rs:249-259`);
punctuation/operator set including `!`-without-`=` rejection (`lex.rs:297-333`); comments to
end of line (`lex.rs:211-213`).

### F4.4.1 [Minor] `1K` is a lexical error; the spec says it is "a number followed by the identifier `K`"

- Spec `SPEC.md:505-507`: "Case matters, so `1k` is 1000 and `1K` is a number followed by
  the identifier `K`."
- Code `lex.rs:215-235`: after a number, any identifier run that is neither a scale suffix
  nor a unit keyword is rejected:
  ``format!("`{run}` cannot follow a number; write a space")``.
- Run: `1K` → ``ERROR `K` cannot follow a number; write a space``.
- The spec contradicts itself: the same section's adjacency rule (`SPEC.md:552-555`) says
  "A unit keyword may follow a number with no whitespace (`3s`, `3kJ`), and nothing else
  may: `3x` is an error". Under that rule `1K` must be an error, and the code is right; the
  sentence at `SPEC.md:506-507` should be reworded (e.g. "`1K` is not 1000").

### F4.4.2 [Minor] `k`, `M`, `G`, `T` are reserved although the spec's reserved set omits them

- Spec `SPEC.md:523`: "These words are reserved and cannot be variable names", where "these
  words" is the unit-keyword table (`second`…`craft`) plus, per `SPEC.md:519-521`,
  `in`/`into`/`from`/`making`/`loop`. `k`, `M`, `G`, `T` are not unit keywords.
- Code `lex.rs:405-409`: `is_reserved_word` returns true for `"k" | "M" | "G" | "T"` as
  well, and `parse.rs:558-560`/`574-576` reject them as targets.
- Run: `M = 1`, `k = 1`, `G = 1`, `T = 1` → "`M` is a reserved word and cannot be a
  variable name". The code's reasoning is documented and sound (`lex.rs:399-404`: these
  words never lex as identifiers after a number), but the spec does not say so. Either the
  spec's reserved list should include the scale suffixes, or `M = 1` should work.

### Matched (§4.1, §4.2, §4.5, §4.6)

- Juxtaposition is one n-ary node evaluated right-to-left (`parse.rs:409-418`,
  `eval.rs:322-352`); there is no number-before-token production; a qualified token is one
  operand.
- `2 :speed-module-2: 3 :productivity-module-2:` doubles the second count — Run confirmed
  with `3 :efficiency-module-1:` (6 efficiency modules).
- Quality is a first-class value: `q = :rare:`; `.level` → `0,1,2,3,5` and `.multiplier` →
  `1 + 0.3×level` (`value.rs:64-78`, `eval.rs:513-519`); Run `:rare: .level` → `2`,
  `:legendary: .multiplier` → `2.5`.
- Every operator rejects a quality except `.level`/`.multiplier` and filters:
  `Value::as_multi` (`value.rs:1636-1646`).
- A quality on the right of juxtaposition is an error — Run `:iron-plate: :rare:` →
  ``ERROR a quality may only be the left operand of juxtaposition`` (`value.rs:1763-1767`).
- Qualified-token lexing is a merge pass over one line's tokens (`lex.rs:302-326`), so it
  cannot cross a newline; `:beacon: :rare: :speed-module-2:` is `:beacon:` + one qualified
  token (Run renders `:beacon::speed-module-2:` for the rare case).
- Alias table `:normal:`/`:common:`/`:quality-1:` … `:quality-5:` → levels 0/1/2/3/5
  (`value.rs:96-118`; `new_eval/mod.rs:1327-1341` test).
- Autocomplete surfaces `q1`…`q5` as `:quality-1:`…`:quality-5:` first
  (`app/completion.rs:524-557`, bonus at `app/completion.rs:471`; test at
  `app/tests/completion.rs:1366-1384`).

---

## §5 Operator semantics

### F5.1.1 [Major] Unary `-` keeps the `resource` and `recipe` reference facets

- Spec `SPEC.md:742-743`: "Both act on the amount coefficient-wise; `per` and the exponents
  are unchanged. A config has no negative form and drops, **as do the reference facets
  (§5.1)**." §5.1 (`SPEC.md:606-614`) lists which operators read a reference facet; unary
  `-` is not among them.
- Code `value.rs:1389-1406`:
  ```rust
  let result = Self { quantity: …, stack_rate: …, stack_capacity: …,
                      machine: …, config: None,
                      resource: self.resource, recipe: self.recipe };
  ```
- Run: `(-:iron-plate:) .:stack-size:` → `100` (the `resource` facet survived, so
  `.:stack-size:` still resolves). Panel of `-:iron-plate:` shows three lines
  (`-1:iron-plate:`, the item glyph, the recipe glyph).
- Minimal repro: `(-:iron-plate:).:stack-size:`.

### F5.3.1 [Major] The spec's own `6 / (2:iron-plate:)` example contradicts its rule

- Spec `SPEC.md:726-728`: "Dividing by a quantity that has a `per` moves that resource into
  the numerator, which is representable only when the dividend's amount is a number:
  `6 / (2:iron-plate:)` is `3:iron-plate:`."
- The rule above it (`SPEC.md:721`) says "Dividing by a single-term bundle moves that
  resource into `per`", and the model (`SPEC.md:672-677`) defines the shape as
  `amount / per`. Under both, `6 / (2 iron-plate)` is `3` **per iron plate**.
- Code `value.rs:417-451` (`reciprocal`) moves the bundle's resource into `per`, i.e.
  follows the rule. Run: `6 / (2:iron-plate:)` → `3/:iron-plate:`; and the rule the sentence
  is actually about works: `6 / (3:iron-plate:/:copper-plate:)` →
  `2:copper-plate:/:iron-plate:`.
- Minimal repro: `6 / (2:iron-plate:)` — the spec's expected `3:iron-plate:` is not
  obtainable and is dimensionally wrong.

### F5.3.2 [Minor] A bundle base with exponent 1 is rejected

- Spec `SPEC.md:740`: "A bundle base is an error **except with exponent `1`**."
- Code `value.rs:457-470`: `let Some(base) = self.amount.number_value() else { return
  Err("a resource bundle cannot be raised to a power") };` — no exponent-1 exemption.
- Run: `(2:iron-plate:) ^ 1` → ``ERROR `^` dropped every facet (left: quantity, right:
  quantity)``; `(2:iron-plate:) ^ 2` also errors (correct).
- Minimal repro: `(2:iron-plate:) ^ 1`.

### F5.3.3 [Major] Filters discard every non-quantity facet

- Spec `SPEC.md:771`: "Exponents and other facets are unchanged." (quality filters); the
  number-filter text (`SPEC.md:747-760`) says only "Terms that fail the comparison are
  dropped".
- Code `value.rs:1497-1519`: both `filter_amount` and `filter_quality` end in
  `Ok(Self::quantity(...))`, which builds a fresh quantity-only `MultiValue`.
- Run: `(2 :foundry:) > 0` → panel `["2:foundry:"]` — the `machine` facet is gone.
- Minimal repro: `(2 :foundry:) > 0`.
- The spec is ambiguous here: §5.1 says only the listed operators read a reference facet
  and "every other operator drops it", so dropping `resource`/`recipe` is consistent;
  "other facets" in §5.3 reads as *all* other facets, which the code does not preserve.

### F5.4.1 [Major] Recipe inference is missing the "resource id" tie-break

- Spec `SPEC.md:840-842`: "several candidates are an error that asks for `making`, **unless
  exactly one of them has the selected resource's id**."
- Code `eval.rs:1474-1525`: after collecting `candidates`, the match is
  ```rust
  match candidates.len() { 1 => Ok(candidates[0]), 0 => Err(…),
                           _ => Err("several recipes match; name one with `making`") }
  ```
  There is no check for a candidate whose recipe id equals the resource id.
- Run: `100 :iron-plate:/s from :foundry:` → ``ERROR several recipes match; name one with
  `making` ``. The foundry can make both `iron-plate` and `casting-iron`, both producing
  iron plates, and exactly one candidate (`iron-plate`) has the selected resource's id, so
  the spec says it should be chosen.
- Minimal repro: `100 :iron-plate:/s from :foundry:`.

### F5.4.2 [Minor] `Unscaled` is multiplied by a time instead of being an error

- Spec `SPEC.md:860`: "`Unscaled` combined with anything is an error."
- Code `value.rs:1181-1210` (`times_seconds`) maps `Extent::Unscaled` to `MachineSeconds`
  ("An unscaled row is one machine, so its machine-seconds are the seconds themselves"),
  and `value.rs:1672-1698` reaches it for `process * time`.
- Run: `(:foundry: making (:normal: :iron-plate:)) * 2s` → a `MachineSeconds` process. Also
  `2 * (:foundry: making …)` works, so the bullet cannot be read literally; it presumably
  means "mixed extents in `+`/`-`". Worth rewording.

### F5.4.3 [Minor] Beacon-profile length is not validated; a short profile silently zeroes rows

- Spec `SPEC.md:926-929`: "Validation happens when a config is attached: … and — once a
  config is used for production — the beacon profile length for the total beacon count."
- Code: `validate_config`/`validate_machine` (`eval.rs:1069-1143`) check slots and allowed
  effects only. The profile is read lazily in `modifiers` (`process.rs:118`):
  `let profile = *beacon.profile.get(index)?;` — an out-of-range index makes `modifiers`
  return `None`, which makes `row_rates` `None` and every derived total disappear.
- Run (1-entry profile, 8 beacons): `making` succeeds and renders, but
  `p = q from (F making :iron-plate:)` → ``ERROR no row of that process involves the
  resource``; `.out.:iron-plate:` says "a row has no rate". The message never mentions
  beacons.
- With a 20-entry profile the same document yields `p.net.:iron-plate:` = `540:iron-plate:/s`
  (B.1 passes).
- Minimal repro: any config whose `total_beacons` exceeds `profile.len()`.

### F5.5.1 [Minor] Dividing a config by a number drops the config facet

- Spec `SPEC.md:1361` (A.2): "C, scalar | scales counts, nonnegative integer factors only";
  `SPEC.md:867-869`: "`*` and `/` by a number scale the module and beacon counts".
- Code: `Value::div` (`value.rs:1701-1715`) turns `x / n` into `x.scale(1/n)`, and
  `Config::scale` (`value.rs:892-904`) calls `as_count(1/n)` (`value.rs:1021-1033`), which
  rejects any non-integer. The `None` is then silently dropped by `MultiValue::scale`
  (`value.rs:1425-1445`).
- Run: `(4 :beacon:) / 2` → panel `["2:beacon:"]` — only the quantity; the `config` facet is
  gone. Compare `2 :beacon: * 2`, whose panel has two lines (quantity + config). Spec A.2
  implies 4 beacons / 2 = 2 beacons with a config of 2.
- Minimal repro: `(4 :beacon:) / 2` (or `… / 4`).

### Matched (§5.1–§5.5)

- Reference facets are read only by the listed operators: `making` (`eval.rs:1145-1180`),
  `.` with a resource (`value.rs:1521-1531`), `:recycling:` (`eval.rs:464-508`),
  `:stack-size:`/`.:stack-size:` (`eval.rs:442-462`), quality filters
  (`eval.rs:903-925`), quality operand (`value.rs:1447-1495`).
- Scaling drops references: `MultiValue::scale` (`value.rs:1425-1445`); Run
  `:stack-size: (3 :iron-plate:)` → error, `3 :stack-size: :iron-plate:` → `300`.
- Juxtaposition rules of the §5.2 table: scalar either side (`value.rs:1772-1779`), stacks
  applied to a pure item/fluid amount (`value.rs:1540-1579`), config-fills-beacon
  (`value.rs:922-947`), module-list merge (`value.rs:908-921`), machine absorbs config
  (`value.rs:1798-1806`), quality operand on the left only.
- Total loss is an error; partial loss is silent (`value.rs:1298-1307`); Run
  `:speed-module-2: 2 :beacon:` → error, `:speed-module-2: + 2 :beacon:` → works.
- `+`/`-` on quantities require matching `per` and exponents and the same amount case
  (`value.rs:285-313`); bundles add term-wise and zero terms vanish (Run
  `2:iron-plate: - 1:copper-plate:` → `-1:copper-plate: + 2:iron-plate:`).
- `*`/`/` exponent addition, bundle×bundle error, two `per`s error, `per` cancellation
  (`value.rs:350-455`); Run `3:iron-plate: * (1:copper-plate:/2:iron-plate:)` with a
  copper-plate recipe token → `1.5:copper-plate:` (the repo's own test uses `:gear:`).
- Division by zero is `f64`: Run `1 / 0` → `inf`, `0 / 0` → `NaN`.
- `^` rules except the bundle-exponent-1 case (F5.3.2): Run `(2s) ^ 2` → `4s²`.
- `.abs`/unary `-` act coefficient-wise; `per`/exponents unchanged (`value.rs:314-336`).
- Number filters compare coefficients and ignore exponents; quality filters drop fluids and
  a dimensionless amount becomes `0` (`value.rs:495-555`); Run
  `(2:iron-plate: - 3:copper-plate:) > 0` → `2:iron-plate:`.
- `.` selection keeps matching terms, `0` when nothing matches, `per` filtered the same way
  (`value.rs:557-591`).
- `making`: machine facet required, recipe facet required, categories must intersect,
  productivity modules gated by `allow_productivity`, `None` recipe quality expands to five
  rows only for item-fed recipes, `Unscaled` vs `Machines(n)` (`eval.rs:1145-1272`).
- `into`/`from`/`in` select input/output/net coefficients; zero magnitude gives an empty
  `Unspecified` process; multi-term quantities scale one row per term; duplicate recipe
  selection errors; extent from `second^-1` vs `0`; a solved process is rejected; hidden
  recipes are still inferable (`eval.rs:1274-1419`); Run
  `100 :iron-plate: from (:foundry: making (:normal: :iron-plate:))` → `53.333*s · …`.
- Process `+`/`-` require the same extent and add row scales; `*`/`/` by a number size the
  process and collapse quality variants; `Machines × time` → `MachineSeconds`;
  `MachineSeconds / time` → `Machines` (`value.rs:1097-1235`); Run
  `2 * (:foundry: making :iron-plate:)` → 1 row, 2 machines.
- Config `+` merges counts per (module, quality) and per (beacon, quality, module list)
  (`value.rs:879-890`); `-` drops the config (via `negate`).
- Config scaling requires a nonnegative integer factor (`value.rs:892-904`).

---

## §6 Facets in depth

### Matched (§6.1, §6.4, §6.5)

- Belt: `speed * 60 * 8` at `Limit(1)`, capacity 8 stacks (`eval.rs:382-385`); Run
  `:transport-belt: :iron-plate:` → `15:iron-plate:/s`, `:express-transport-belt:` → 45/s
  (540/s for 3 stacked belts, B.1).
- `:stacked-belt:` collapses every stack facet from `Limit(1)` to the setting, clamped 1–4
  (`value.rs:1581-1607`, `eval.rs:834-836`); a fluid rate errors (Run
  `:stacked-belt: (:pump: .rate)` → "needs a rate whose only stack limit is one").
- Inserter: one transfer per cycle, cap = base (1 / 2 bulk / 2+bonus stack) + research,
  clamped to the registry's levels; `hand_size` unused (`eval.rs:840-862`); Run
  `:inserter: .rate` → `0.857…₁/s`.
- Container: slots at `Full`, or fluid volume (`eval.rs:398-408`); pump:
  `pumping_speed*60` (`eval.rs:409-411`); offshore pump reaches the same rule; barrel
  capacity 50 with the id deciding (`eval.rs:393-397`, `828-832`).
- Zero stack terms removed but the facet stays (`value.rs:728-737`); `format_stacks` prints
  `0:stack-size:/s` for an empty item map (`format.rs:373-407`).
- Process: `extent` + rows keyed by (machine, machine quality, config, recipe, recipe
  quality) with the scale in the extent's unit (`value.rs:1035-1075`); `Unspecified` is the
  identity under `+` (`value.rs:1097-1106`); recipe-quality variants are alternatives for
  `.machines` (`process.rs:245-262`); per-row rates are derived, not stored
  (`process.rs:36-95`); selectors return the extent's units (`eval.rs:629-739`).
- `ResourceRef`/`RecipeRef` carry identity only (`value.rs:676-687`).

### F6.2.1 [Minor] Zero module counts are not removed from a config

- Spec `SPEC.md:913-915`: "The module list is sorted by (module id, quality) and holds a
  count per entry. **Zero counts are removed.**"
- Code: `Config::add` (`value.rs:879-890`) and `Config::scale` (`value.rs:892-904`) never
  drop zero entries; there is no canonicalization for `Config`.
- Run: `0 :speed-module-2:` → single-line `0`, panel `["0", ""]` — the config line renders
  as the empty string because `format_module_glyph` repeats the glyph `0` times
  (`format.rs:307-319`).
- Minimal repro: `0 :speed-module-2:` (panel shows an empty line).

### F6.3.1 [Minor] No idle power, and power is not rounded up to whole machines

- Spec `SPEC.md:966-968`: "Active power is `energy_usage × multiplier`; **idle power is
  included and rounded up to whole machines for a machine count**."
- Code `process.rs:181`: `power_w: machine.energy_usage_w * consumption` — no drain/idle
  term exists at all (`hactorio_data::Machine`, `registry.rs:143-158`, has only
  `energy_usage_w`), and `.power` multiplies by the raw count (`eval.rs:565-579`).
- Run: `(2.5 :foundry:).power` → `5MW` and `.machines` → `2.5`; the spec's rule would
  report three machines' power plus idle draw.
- Minimal repro: `(2.5 :foundry:).power`.

### Matched (§6.2, §6.3)

- Config ordering key is (module id, quality) (`value.rs:837-872`); a beacon group is keyed
  by (beacon, beacon quality, module list) (`value.rs:837-842`); an empty config renders
  `0` only when it is the whole value (`format.rs:36-41`).
- Validation at attachment: machine slots and allowed effects, beacon slots and
  `allowed_effects` (`eval.rs:1069-1143`); a vanilla beacon refuses productivity/quality
  modules (Run `:beacon: :productivity-module-1:` → error).
- Several beacon types and qualities are representable; the profile depends only on the
  total count (`process.rs:97-191`).
- Machine modifiers: module effects summed and filtered by allowed effects; beacon effects
  per group × count × `distribution_effectivity` × quality × profile[total−1]; speed =
  machine speed × quality multiplier × (1 + speed effect); productivity = base + modules +
  research, capped at +300%, zero when the recipe forbids it; consumption ≥ 0.2; pollution =
  base × consumption × (1 + pollution effects) (`process.rs:97-191`, `193-212`). Run:
  `(:legendary: :foundry:).:speed:` → `10`, `(:foundry: :legendary::speed-module-2:).:speed:`
  → `7`, `(2 :foundry:).power` → `4MW`.
- Quality modules split products as `p^n` from the recipe's own quality
  (`process.rs:346-365`); Run `p.out.:iron-plate:` from a 25%-quality foundry → five
  qualities with shares 0.25 / 0.0625 / … .
- `.productivity` returns `1 + bonus` (`eval.rs:580-590`, `eval.rs:715-721`).

---

## §6.6 Selectors

### F6.6.1 [Major] `.:time:` is not implemented (NOT IMPLEMENTED)

- Spec `SPEC.md:1068`: "| `.:time:` | process, `MachineSeconds` | machine-seconds as a time
  quantity |"; `SPEC.md:1089-1091`; Appendix B.2 (`SPEC.md:1408`) asserts `p.:time:`.
- Code: neither selector match arm handles `Signal::Time` — the `MultiValue` arm
  (`eval.rs:527-623`) has `Item`, `Fluid`, `StackSize`, `Speed`, `ModuleSlots` and falls
  through to `other => Err("needs a facet this value does not have")` (`eval.rs:619-623`);
  `select_process` (`eval.rs:629-739`) has no `Time` arm and ends in the same catch-all
  (`eval.rs:735-738`). `property_is_available` (`eval.rs:1041-1063`) does not list `time`
  either, so completion never offers it.
- Run: `p = 100 :iron-plate: from (:foundry: making (:normal: :iron-plate:))` then
  `p.:time:` → ``ERROR `:time:` needs a facet a process does not have``.
- Minimal repro: B.2's own document.

### F6.6.2 [Major] `.power` / `.energy` / `.:electric:` on an item are not implemented (NOT IMPLEMENTED)

- Spec `SPEC.md:1063`: "| `.power` | item | the prototype's `power_w` × coefficient |";
  `SPEC.md:1065`: "| `.energy` | item | stored energy × coefficient |"; `SPEC.md:1067`:
  "| `.:electric:` | item | `power_w` when present, else stored energy |".
- Code: `Property::Power` in the `MultiValue` arm requires a `machine` facet
  (`eval.rs:565-579`); `Property::Energy` and `Signal::Electric` are handled **only** in
  `select_process` (`eval.rs:667-677`, `691-708`) and fall into the catch-all for a
  `MultiValue`. The registry does carry the data (`Item.power_w`, `Item.stored_energy_j`,
  `postprocess.rs:427-451`), so only the evaluator is missing.
- Run:
  - `:small-lamp: .power` → ``ERROR `.power` needs a machine facet (quantity resource)``
  - `:accumulator: .energy` → ``ERROR `energy` needs a facet this value does not have``
  - `:accumulator: .:electric:` → same
- Minimal repro: `:accumulator: .energy`.

### F6.6.3 [Nit] `.abs` keeps the `config` facet

- Spec `SPEC.md:1072`: "| `.abs` | any non-process | absolute values; **config drops** |";
  `SPEC.md:742-743` repeats "A config has no negative form and drops".
- Code `value.rs:1408-1424`: `config: self.config.clone()` (and `resource`/`recipe` are
  kept as well).
- Run: `(2 :beacon:).abs` → panel `["2:beacon:", "2:beacon:"]` (quantity + config).
- Minimal repro: `(2 :beacon:).abs`.

### Matched (§6.6)

`in` after a dot is the property (`parse.rs:509-511`; the highlighter suppresses the keyword
class after a dot, `syntax.rs:137-146`); the property name table is exactly the spec's list
plus `.prod` (`parse.rs:62-88`); a `.` takes any expression naming a resource
(`parse.rs:499-524`, `eval.rs:303-320`); `.rate`/`.capacity` return that facet alone
(`eval.rs:545-564`); `.machines` on a machine (absent count = 1) and on Unscaled/Machines
processes, error on MachineSeconds (`eval.rs:538-544`, `633-640`); `.crafts`
(`eval.rs:644-654`); `.productivity` shared-or-error (`eval.rs:580-590`, `715-721`);
`.pollution` per minute vs total (`eval.rs:709-714`, `process.rs:298-313`); `.power` on a
process errors for MachineSeconds (`eval.rs:655-666`); `.energy` only for MachineSeconds
(`eval.rs:667-677`); `.:electric:` switches by extent (`eval.rs:691-708`); `.:speed:` machine
and process (`eval.rs:591-601`, `684-690`); `.:module-slots:` (`eval.rs:602-617`);
`.:stack-size:` on a resource (`eval.rs:442-462`); `.:item:`/`.:fluid:` on quantity and
process (`eval.rs:529-534`, `678-683`); `.level`/`.multiplier` (`eval.rs:513-519`);
`.in`/`.out`/`.net` (`eval.rs:641-643`); a property with no meaning is an error, never a
silent zero (every arm returns `Err`).

Note: the §6.6 table lists `.:item:` and `.:fluid:` twice (`SPEC.md:1075-1076` and
`SPEC.md:1080-1081`) — harmless duplication.

---

## §7 Display

### Matched (§7.2–§7.6)

- Numbers: positional, `max_decimals` fraction digits for `|x| ≥ 1` and significant digits
  below, trailing zeros trimmed, `-0` → `0`, sub-threshold → `0`, `NaN`/`inf` as written
  (`format.rs:479-506`). Run reproduces every row of the §7.2 table:
  `123456.7891` → `123456.789`, `1/3` → `0.333`, `0.0001234` → `0.000123`, `2.0000` → `2`,
  `0.000000000001` → `0`.
- Metric prefixes on item/fluid amounts, rates and stack rates when `metric_prefixes` is on;
  SI always for J/W; never on dimensionless numbers, time, or craft (`format.rs:508-526`,
  `format_si` at `528-534`). Run with `metric_prefixes=false` → `1500` instead of `1.5k`.
- Time: `180 s` → `3 m`, `15/s` → `900/m` in minute mode (`format.rs:334-349`; Run).
- Round-trippable machines and configs: `:foundry:`, `2:foundry:`,
  `:foundry:(:efficiency-module-3:)`, module repetition, beacon count as a coefficient,
  groups joined by `+` (`format.rs:256-319`); Run re-evaluating
  `:foundry:(:speed-module-2::speed-module-2:+2:beacon:)` yields an equal value.
- Quantity rendering: coefficient with no separating space, quality glyph omitted at normal,
  bundle as a sum, denominator after `/`, `1.5kJ` / `2MW` / `craft` (`format.rs:321-354`).
- Stack limits: `Full` first, limits descending, subscript for a limited cap, `Full` with no
  subscript (`value.rs:698-716`, `format.rs:373-407`); Run `3.2…/s + 5.01…₁/s` matches the
  §7.6 example.

### F7.1.1 [Minor] The single-line result never appends `…`

- Spec `SPEC.md:1139`: "| single-line | the first facet present, then `…` if any other facet
  is present |" and `SPEC.md:1143-1146`: "The `…` makes the loss visible rather than hiding
  it."
- Code: `format_multi` (`format.rs:23-58`) returns the first facet and nothing else;
  `single_line_result` (`app/results/mod.rs:157-167`) just calls `format_value`. The only
  `…` in the UI is width truncation (`app/editor.rs:439-467`).
- Run: `2:beacon:` (quantity + config) → single-line `2:beacon:` with no `…`, while the
  panel shows two lines.
- Minimal repro: `2:beacon:`.

### F7.1.2 [Nit] Single-line facet order puts stack facets before `config`

- Spec `SPEC.md:1118`: canonical order is `machine`, `quantity`, `config`, `stack_rate`,
  `stack_capacity`, `recipe`.
- Code: `format_multi` checks `stack_rate`/`stack_capacity` (`format.rs:30-35`) before
  `config` (`format.rs:36-41`), while `format_facets` uses the spec order
  (`format.rs:66-103`).
- No operator currently produces a value holding both a stack facet and a config, so this is
  unreachable today; it becomes visible the moment one is added.

### F7.5.1 [Major] Multi-term quantities repeat the unit once, and the rendering does not parse back

- Spec `SPEC.md:1210-1214`: "When a quantity has several resource terms, the unit suffix is
  **repeated on every term**: `2:iron-plate:/s + 3:copper-plate:/s`. This parses back to the
  same quantity, because addition requires matching exponents, so distributing the suffix
  cannot change the value."
- Code `format_quantity` (`format.rs:321-354`): `format_amount` joins the terms with ` + `
  (`format.rs:356-371`) and the suffix is appended once after the whole amount.
- Run: `1:iron-plate:/s + 2:copper-plate:/s` → `2:copper-plate: + 1:iron-plate:/s`. Pasting
  that back gives ``ERROR `+` dropped every facet (left: quantity, right: quantity)``
  (units differ), so the §7.5 round-trip promise is broken for any multi-term quantity with
  a unit.
- Minimal repro: `1:iron-plate:/s + 2:copper-plate:/s`, then evaluate its rendering.

### F7.7.1 [Minor] The process panel hardcodes `craft/s`

- Spec `SPEC.md:1291-1293`: "Units follow the extent: machines and crafts **per the
  configured time unit** for a `Machines` process, machine-seconds and craft totals for a
  `MachineSeconds` one."
- Code `format.rs:166-172`: `format!("{} craft/s", …)` for every rate process; only the
  in/out aggregate lines use `time_unit_name` (`format.rs:178-198`).
- Run with `time_unit = minute`:
  `["power 576MW", "pollution 0/m", "288<foundry> · 360 craft/s", "in", "21.6k<ore>/m", …]`
  — the panel mixes `craft/s` with `/m`.
- Minimal repro: minute mode + any `Machines` process.

### F7.8.1 [Major] `item_rate_belt` and `stacked_belts` do not exist (NOT IMPLEMENTED)

- Spec `SPEC.md:1302-1307`: the table lists `item_rate_belt` ("when set, item rates are
  rendered as belt counts of this tier") and `stacked_belts` ("whether those belt counts use
  the stacked form"), and D27 (`SPEC.md:1485`) reaffirms them.
- Code: `Settings` (`settings.rs:58-77`) has `max_decimals`, `metric_prefixes`, `time_unit`,
  `belt_stacking`, `inserter_capacity`, `research_levels` — no belt-rendering settings. The
  display settings UI offers only Time unit / Decimals / Metric prefixes
  (`app/mod.rs:666-681`), and `format_quantity`/`format_process` never consult a belt tier.
- There is no code path to verify; grep for `item_rate_belt`/`stacked_belts` returns nothing
  outside `SPEC.md`.

---

## §8 Diagnostics

### Matched

- Every error carries a span and a message: `ParseError { span, message }`
  (`parse.rs:21-25`), `Output::Error { message, span }` (`eval.rs:20-29`).
- Four kinds exist: lexical ("unterminated token", "unknown prototype", "cannot follow a
  number"), syntactic ("expected `)`", "a loop count must be a non-negative integer"),
  facet loss, semantic ("a denominator resource is only representable with exponent one").
- Facet-loss errors name both operands' facets and nothing else does
  (`value.rs:1298-1307`); Run `1 :iron-plate: + 1s` →
  `` `+` dropped every facet (left: quantity, right: quantity) ``.
- Partial drops are silent (P3) — the panel simply shows the remaining facets.
- An error never stops evaluation (`eval.rs:136-215`).
- A cancelled evaluation reports nothing (`eval.rs:96-102`; `evaluator.rs:58-60`).

### F8.1 [Major] A poisoned name re-raises with the reading line's span

- Spec `SPEC.md:1326-1328`: "A definition that fails binds its name to the error. Reading
  the name re-raises the original error **with its original span**, so the message points at
  the line that failed rather than the line that read it."
- Code: the environment stores only a `ValueError` (message, no span) —
  `type Env = BTreeMap<String, Result<Value, ValueError>>` (`eval.rs:86`) — and every
  expression/definition error is attached with `span: self.line_span(line)`, i.e. the
  *current* line's `0..len` (`eval.rs:158`, `167`, `175`, `212`).
- Run:
  ```
  x = 1 :iron-plate: + 1s
  y = x
  ```
  line 0 → error with span `0..23`; line 1 → the same message with span `0..5` (the
  `y = x` line), not line 0's span.
- Minimal repro: the two-line document above.

---

## Appendix A / B

- Appendix A: every row was located in code — A.1 `Q,Q` (`value.rs:285-313`), `T,T`/`K,K`
  (`value.rs:739-752`), `M,M` (`value.rs:1008-1019`), `C,C` (`value.rs:879-890`), `P,P`
  (`value.rs:1097-1134`); A.2 `scalar,any` (`value.rs:1672-1700`, `1701-1715`), `Q,Q`
  (`value.rs:350-455`), `T,Q`/`K,Q` (`value.rs:1343-1363`, `1365-1378`), `M,scalar`,
  `C,scalar` (F5.5.1), `P,scalar` (`value.rs:1155-1179`); A.3 (`value.rs:1380-1387`);
  A.4 (`eval.rs:903-925`, `1145-1419`). No operator exists in code that the matrix omits.
- B.1 passes with a realistic 20-entry beacon profile: `q.:iron-plate:` = `540:iron-plate:/s`
  and `p.net.:iron-plate:` = `540:iron-plate:/s`, `p.machines` = `19.2`, `p.:electric:` =
  `791.04MW`.
- B.2 fails at `p.:time:` (F6.6.1); the other three lines behave as specified.
- B.4 passes: `a == b` for the two foundry spellings, and re-evaluating a rendered machine
  gives an equal value.
- FB.1 [Minor] B.3 says "`p` stays a `Machines` process across iterations, so `+=` is
  valid" (`SPEC.md:1435-1436`), but its own document is
  `p = 1000 :iron-plate: into (R making q)` — an *amount*, which §5.4 (`SPEC.md:834-835`)
  says gives `MachineSeconds`. Code gives `MachineSeconds` (Run:
  `1000 :iron-plate: into (R making q)` → `31.25*s · …`), and `+=` still works because both
  sides share the extent. The example's claim is wrong, not the code.

---

## Could not verify

- **`EXPRESSION_LANGUAGE.md`**: absent from the repository, so the "old implementation"
  comparisons in `SPEC.md` (e.g. `SPEC.md:1103-1104`, D22, D39) cannot be checked.
- **Real game data**: no `data-raw-dump.json`/cache is present, so game-accurate constants
  (`beacon.profile` contents, `distribution_effectivity`, real recipe times, inserter
  research tables) were exercised against a synthetic dump with the same shape. Numbers
  quoted above are therefore structural, except B.1's 540/s, which follows from
  `speed × 60 × 8` and is data-independent.
- **The egui surfaces** (results column layout, multiline panel placement, token-tip
  content, completion popup ordering in situ) were verified by reading
  `app/results/*`, `app/editor.rs`, `app/completion.rs` and their tests, not by driving the
  GUI.
- **`item_rate_belt` / `stacked_belts`**: nothing to run; absent from the codebase
  (F7.8.1).
