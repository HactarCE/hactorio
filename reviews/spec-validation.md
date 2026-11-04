# SPEC.md — internal validation (coherence, examples, grammar, coverage)

Scope: `SPEC.md` read in full (1503 lines), judged as a language definition on
its own terms. No source code was read or run; every claim below cites
`SPEC.md:LINE`. Derivations are by hand from the stated rules.

Severity: **Blocker** = the spec is self-contradictory or an acceptance example
cannot hold; **Major** = real ambiguity / divergence a user or implementer hits;
**Minor** = missing detail or loose wording with a behavioural difference;
**Nit** = editorial.

---

## §1 Design principles

- **Minor — P5 vs §6.3.** P5 says "Quality is never a distribution over
  outcomes" (`SPEC.md:66-72`); §6.3 says "Each product is split across those
  qualities, which is why `.out` reports one row per quality"
  (`SPEC.md:953-955`) with outcome percentages. Either P5 needs a
  process-level carve-out or §6.3 is unfounded.

## §2 Program model

- **Minor — compound-assignment operand types.** `*=`/`/=` "take a
  dimensionless number" (`SPEC.md:112-114`), but `+=`/`-=` operands are
  unstated (rate, amount, or unit-bearing quantity), and the behaviour of any
  assignment to an `Unscaled` (`SPEC.md:981`) or `Unspecified`
  (`SPEC.md:985`) process is unstated.
- **Minor — no rule for malformed blocks.** §2.1 (`SPEC.md:82-95`) has no rule
  for `}` without a header, an unclosed `{`, or a parse error in the `loop`
  header (whether the body still runs); §2.5 (`SPEC.md:138-144`) covers
  statements only, and a loop header is a statement.
- **Nit — comment vs token.** "`#` outside a `:token:`" (`SPEC.md:91`) implies
  `#` may occur *inside* a token, and a trailing comment after `{` conflicts
  with "A `{` must be the last token on its line" (`SPEC.md:93`).

## §3 Value model

- **Minor — quality × quality is undefined.** §3.3 says fluids and non-resource
  units are "unaffected" and "A value with no item terms is unchanged"
  (`SPEC.md:297-302`), which leaves `:rare: :epic:` and `q1 q2` (two stored
  qualities juxtaposed) with no stated result.

## §4 Grammar

- **Minor — `.5` vs the `.` operator.** "`.5` are numbers" (`SPEC.md:502-503`)
  plus `.` as punctuation means `v.5` lexes as `v` followed by `0.5`, i.e. a
  juxtaposition chain that silently scales `v` by 0.5. No maximal-munch rule
  states the tie-break, and §4.4 calls adjacency "the only whitespace-sensitive
  rule" (`SPEC.md:554`).
- **Minor — `1MJ` has two readings.** The scale suffix rule
  (`SPEC.md:505-507`) and the unit table (`SPEC.md:517`) both match `MJ`/`TW`;
  the values coincide, but which rule wins is unstated, and `1kJ` (unit only)
  behaves differently from `1MJ` (both).
- **Minor — signal selectors have no precedence level.** §4.3 knows only
  `.property` (3) and `.` with a resource operand (4) (`SPEC.md:453-455`),
  while §6.6 introduces a third class ("a signal token such as `.:stack-size:`
  is a signal selector", `SPEC.md:1041-1042`) used as `.:stack-size:`,
  `.:speed:`, `.:time:`, `.:electric:`, `.:module-slots:`, `.:item:`,
  `.:fluid:`. Which level they occupy decides `p.:item:.abs` and
  `p.out.:item:`.
- **Minor — level 5 is misnamed and inverted without justification.**
  `SPEC.md:456` calls level 5 "quality filters", but §5.3:747-763 defines a
  *number* filter with the same tokens. It also binds tighter than *all*
  arithmetic (levels 8-11), so `q + r < :rare:` filters `r` only and
  `2 * q > 0` parses as `2 * (q > 0)`; the two other inversions are justified
  (`SPEC.md:471-486`), this one is not.
- **Nit —** left-associative filters accept `a < b < c` (a filter applied to a
  filtered quantity), which no rule mentions.

## §5 Operator semantics

- **Minor — "the only common one" does not choose a quality.**
  `SPEC.md:853-856`. `2 :foundry: making :transport-belt:` yields five rows
  whose recipe differs only in quality (`SPEC.md:994-1000`), so "the only
  common one" is satisfied five times over; which row is kept (and thus which
  quality the machines are counted at) is unstated.
- **Minor — `-` and `.abs` are defined only for quantity and config.**
  `SPEC.md:742-745`. `T`, `K`, `M`, `P` are unstated (`-belt.rate`, `-p`).
- **Minor — self-cancelling division.** Dividing by a single-term bundle whose
  resource equals the numerator's term (`3:iron-plate:/:iron-plate:`) is
  unstated: cancel, `per`, or error (`SPEC.md:722-729`).
- **Minor — §5.1's table misuses its own columns.** The quality-operand row
  (`SPEC.md:614`) lists magnitude facets under "Reads" and a top-level kind
  under "Operator", in a table whose stated subject is reference facets
  (`SPEC.md:604-605`); §5.1:610 also claims `.` reads the `resource` facet,
  whereas §5.3:778 and §6.4:1003-1006 select *quantity terms and process rows*
  (a `p.out` quantity has no `resource` facet at all).

## §6 Facets in depth

- **Minor — empty config rendering.** "An empty config is present but empty. It
  renders as `0` only when it is the whole value (§7.5)"
  (`SPEC.md:918-919`); §7.5's config grammar has no empty production and no
  `0` (`SPEC.md:1232-1234`), so an empty config renders as the empty string.
- **Minor — undefined terms in §6.6.** "modifier power" (`SPEC.md:1060`) and
  "the prototype's `power_w` × coefficient" for items (`SPEC.md:1063`) are
  undefined; `.EXPR`'s "names a resource" (`SPEC.md:1079`) is undefined
  (quantity term? `resource` facet?), which decides `p . :stack-size:`,
  `p . q` with `q` a process, `p . 3`.
- **Minor — duplicated selector rows.** `.:item:` / `.:fluid:` appear twice
  with different wording — "the item terms only" (`SPEC.md:1074-1075`) and "the
  item rows" (`SPEC.md:1080-1081`); `.EXPR` (1079) overlaps `.` selection
  (1003-1006).

## §7 Display

- **Minor — an undefined row notation.** `288<foundry>` and
  `53.333s*<foundry>` (`SPEC.md:1284`) are defined nowhere and conflict
  with §7.5's machine syntax (`2:foundry:`). Also, "single-line result" is
  defined twice with different content: "the first facet present, then `…`"
  (`SPEC.md:1139`) and "the machine total, then the net rows"
  (`SPEC.md:1275`).

## Appendix B — acceptance examples

- **Minor — B.1's exact number depends on a setting.** "exactly
  `540:iron-plate:/s`" (`SPEC.md:1391`) requires `belt_stacking = 4`
  (`SPEC.md:884-886`, `SPEC.md:1304`), whereas Appendix B's framing says the
  exact values are those "derivable from registry constants"
  (`SPEC.md:1376-1378`).
- **Nit — two T1 assertions are not invariants.** "`p.machines` is a machine
  count" (`SPEC.md:1387`) and "`p.:electric:` # watts" (`SPEC.md:1388`) assert
  no property, so they cannot fail; Appendix B promises "the invariants its
  results must satisfy" (`SPEC.md:1375`).

## Appendix C — decision log

- **Minor — D20's "unobservable" claim is false.** `SPEC.md:1478` says the
  `-`-tighter-than-`+` inversion "is unobservable because a `Quantity` is an
  additive bundle so grouping cannot change a result". With `f64`
  coefficients, grouping does change the result: under the table
  `0.1 + 0.2 - 0.3` is `0.1 + (0.2 - 0.3)`, under ordinary left associativity
  it is `(0.1 + 0.2) - 0.3` — different bit patterns, and the undefined
  approximate-zero threshold decides whether the difference is visible.
- **Minor — D44's "naming them (`:recycling:`)" is inconsistent with §4.4**
  (`SPEC.md:1502`). `:recycling:` is a prefix operator that reads a `resource` facet
  (`SPEC.md:534`, `SPEC.md:611`), so it cannot name a hidden recipe on its own;
  B.3:1432-1434 asserts it can ("The recycling recipe has to be named with
  `:recycling:`"). Whether a bare `:recycling:` token exists is unresolved.
- **Minor — the log omits decisions that are normative elsewhere.** No entry
  for the filter-vs-arithmetic precedence inversion (`SPEC.md:456`), the
  `stack_rate`→item conversion (`SPEC.md:636`), `Unspecified`
  (`SPEC.md:985`), the approximate-zero threshold (`SPEC.md:345`), or which
  selector is assignable (`SPEC.md:109`), although the log presents itself as
  the record of decisions.

## Cross-references

- **Minor — every citation resolves, one misdescribes its target.** All 21
  distinct citations (`§2.2`, `§2.5`, `§2.7`, `§3.2`-`§3.4`, `§4.1`, `§4.2`,
  `§4.5`, `§4.6`, `§5`, `§5.1`-`§5.4`, `§6.4`, `§7.2`-`§7.6`) point at
  existing sections. Wrong claim:
  - `SPEC.md:610` (§5.1) says `.` with a resource operand reads the `resource`
    facet; the operator is defined on quantity terms and process rows
    (`SPEC.md:778`, `SPEC.md:1003-1006`), and `p.out.:iron-plate:` has no
    `resource` facet at all.

## Operator × facet coverage table

Facets: **Q** `quantity`, **T** `stack_rate`, **K** `stack_capacity`,
**M** `machine`, **C** `config`, **R** `resource` (reference), **Rt** `recipe`
(reference); top-level kinds **P** `process`, **Ql** `quality`.

Legend: `✓` defined by a stated rule · `↓` stated to drop the facet (P3,
§5.1's blanket rule, or Appendix A's blanket rule) · `✗` stated error · `?`
not stated anywhere · `·` not applicable.

`↓` for the reference facets R and Rt comes from §5.1's blanket rule
(`SPEC.md:604-605`) and holds only for the operators §5.1 does not list; the
six it does list read or rewrite them instead.

| Operator | Q | T | K | M | C | R | Rt | P | Ql | Rules |
|---|---|---|---|---|---|---|---|---|---|---|
| juxtaposition | ✓ | ✓ | ✓ | ✓ | ✓ | ↓ | ↓ | ? | ✓/✗ | 629-642 |
| `+` | ✓ | ✓ | ✓ | ✓ | ✓ | ↓ | ↓ | ✓ | ✗ | 1340-1349 |
| `-` | ✓ | ✓ | ✓ | ✓ | ↓ | ↓ | ↓ | ✓ | ✗ | 1340-1349 |
| `*` | ✓ | ✓ | ✓ | ✓ | ✓ | ↓ | ↓ | ✓ | ✗ | 1351-1360 |
| `/` | ✓ | ↓ | ✓ | ✓ | ✓ | ↓ | ↓ | ✓ | ✗ | 1351-1360 |
| `^` | ✓ | ↓ | ↓ | ↓ | ↓ | ↓ | ↓ | ? | ✗ | 1363-1365 |
| filter, quality RHS | ✓ | ? | ? | ? | ? | ? | ? | ? | ✗ | 613, 761-775 |
| unary `-` | ✓ | ? | ? | ? | ↓ | ↓ | ↓ | ? | ✗ | 742-745 |
| `.abs` | ✓ | ✓ | ✓ | ✓ | ↓ | ↓ | ↓ | ✗ | ✗ | 742-745, 1072 |
| `.` resource | ✓ | ? | ? | ? | ? | ? | ? | ✓ | ✗ | 610, 777-786, 1003-1006 |
| `.:item:` / `.:fluid:` | ✓ | ? | ? | ? | ? | ? | ? | ✓ | ✗ | 1074-1075, 1080-1081 |
| `.:stack-size:` | ✗ | ? | ? | ? | ? | ✓ | ? | ? | ✗ | 1073 |
| `:stack-size:` (prefix) | ✗ | ✗ | ✗ | ? | ? | ✓ | ✗ | ? | ✗ | 613-620 |
| `:stacked-belt:` | ✗ | ✓ | ✓ | ? | ? | ↓ | ↓ | ? | ✗ | 886-895 |
| `:recycling:` | ? | ? | ? | ? | ? | ✓ | ↓ (writes) | ? | ✗ | 611 |
| `making` | ✗ | ✗ | ✗ | ✓ | ✓ | ↓ | ✓ | ✗ | ✗ | 792-808 |
| `in` / `into` / `from` | ✓ | ? | ? | ✓ (RHS) | ? | ↓ | ↓ | ✓ (Unscaled only) | ✗ | 811-847 |
| process properties | · | · | · | · | · | · | · | ✓ | · | 1050-1082 |
| quality operand | ✓ | ↓ | ↓ | ✓ | ✓ | ✓ | ✓ | ✗ | ? | 257-303 |

### `?` cells: combinations no rule covers

- **Juxtaposition with a process** (`p` column): §3.4 says a process "never
  combines" with anything (`SPEC.md:309-313`) but §5.2 has no row, so whether
  `p q` drops or errors is unstated (P3 would make it an error only if nothing
  survives — a process has no facets to survive).
- **`*`, `/`, `^`, unary `-` on `stack_rate`/`stack_capacity` in the "wrong"
  order**: A.2 only has `T, Q(time)` and `K, Q(time)` (`SPEC.md:1357-1358`);
  `Q * T`, `T * T`, `K * T`, `K * K` are unlisted, while §5.2's
  `any | scalar` row exists for juxtaposition only.
- **Filters on T, K, M, C, P**: §5.3 defines `QUANTITY CMP RHS` only
  (`SPEC.md:747-749`); §5.1 adds that reference facets are *read*
  (`SPEC.md:613`) but never says what the result is when the left operand has
  no quantity terms (e.g. `:iron-plate: < :rare:` after scaling, or
  `:foundry: < :rare:`).
- **`.` selection on T, K, M, C**: §5.3 (quantity) and §6.4 (process) only.
- **`.:item:`/`.:fluid:` on M, C, R**: unstated; `:item:` is documented as
  "only valid after a `.`" (`SPEC.md:531-533`) yet its operand set is
  "quantity or process".
- **`.:stack-size:` on a process, or where the item term is absent but the
  reference names a fluid**: `SPEC.md:1073` says "naming an item"; fluid
  behaviour unstated.
- **`:stacked-belt:` on a machine or a config**: "requires every stack facet
  to be at `Limit(1)`" (`SPEC.md:886-890`); with no stack facets at all P3
  makes it an error, which is unstated.
- **`:recycling:` on a scaled or non-item value**: `SPEC.md:611` reads
  `resource`, and §5.1's scaling rule (`SPEC.md:616`) means
  `:recycling: 3 :iron-plate:` has no `resource` facet, so it must error —
  never stated.
- **`in`/`into`/`from` with `Machines(n)`, `MachineSeconds`, or `Unspecified`
  on the right**: `SPEC.md:812-813` lists only `Unscaled` or a machine; the
  other three are neither allowed nor declared errors.

### Conflict cells the table cannot express

| Cell | Conflict |
|---|---|
| `+`/`-`/`*` with `Unspecified` | `SPEC.md:831-832` (identity) vs `SPEC.md:850-851` (same extent kind required) |
| `.` resource with Q vs R | `SPEC.md:610` (reads `resource`) vs `SPEC.md:778` (matches terms) — the two disagree for any quantity that has terms but no reference facet |
| `:stack-size:` on Q | `SPEC.md:619-620` (error when `resource` was destroyed by scaling) vs `SPEC.md:616-618` (scaling destroys it silently) — the error is a *consequence*, which no P3 rule authorises |

---

## Checked and clean

- §4.1's chain examples all follow from right-associative juxtaposition:
  `:beacon: 2 :speed-module-2:` ≡ `:beacon: n :speed-module-2:`
  (`SPEC.md:359-369`), and `2 :speed-module-2: 3 :productivity-module-2:`
  = 2 speed + 6 productivity (`SPEC.md:372-375`).
- §4.3's documented groupings are producible by the table:
  `3 :iron-plate: * 1 :gear: / 2 :iron-plate:` (`SPEC.md:477-478`),
  `:foundry: (2 :efficiency-module-3:) making :iron-gear-wheel:`
  (`SPEC.md:493-494`), `-2 ^ 2` = `-(2 ^ 2)` (`SPEC.md:483-486`).
- §4.2's `q :speed-module-1: :efficiency-module-1:` really does error via
  §3.3's "two module entries" rule (`SPEC.md:433-435`).
- §3.3's `:rare: :speed-module-3: :speed-module-3:` promotes only the first
  module, consistently with §4.5 (`SPEC.md:286-288`).
- `v.:common::iron-plate:` (`SPEC.md:243`) lexes consistently: the pair
  `:common::iron-plate:` is one qualified token (§4.5) and the `.` at level 4
  selects it, which is exactly §3.2's stated meaning. Same for the `:rare: 2
  :iron-plate:/s` line (`SPEC.md:301`): `:rare:` is a quality value (no
  prototype token follows it), `/` is the level-8 operator and `s` the time
  keyword, so the chain means "two rare iron plates per second" as commented.
- §7.2's rounding table (`SPEC.md:1157-1162`) and §7.4's time examples
  (`SPEC.md:1175-1180`) are arithmetically correct.
- The grammar has no left-recursive production: juxtaposition is an n-ary node
  folded right (`SPEC.md:353-356`), postfix `.property` sits above it, and
  binary levels are all left-associative except `^`.
- `%` never appears as a token or in any expression example; the only `%`
  occurrences are prose (`SPEC.md:952`, `SPEC.md:963`), so no finding.
- No private-use-glyph token appears in any example; every `:...:` token in
  Appendix B resolves to a registry name, a signal token, or a quality token.

## Could not verify

- `SPEC.md:883`'s belt constant (`belt speed * 60 * 8` = 45 items/s) and
  `SPEC.md:892-896`'s inserter capacities are game-data claims; only their
  internal consistency with B.1's arithmetic was checked here.
- Whether the registry's quality levels are `0,1,2,3,5` (`SPEC.md:583`) — not
  checkable without the dump; the `.multiplier` formula
  (`SPEC.md:405`) is consistent with those levels.
