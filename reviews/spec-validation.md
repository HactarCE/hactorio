# SPEC.md — internal validation (coherence, examples, grammar, coverage)

Scope: `SPEC.md` read in full (1503 lines), judged as a language definition on
its own terms. No source code was read or run; every claim below cites
`SPEC.md:LINE`. Derivations are by hand from the stated rules.

Severity: **Blocker** = the spec is self-contradictory or an acceptance example
cannot hold; **Major** = real ambiguity / divergence a user or implementer hits;
**Minor** = missing detail or loose wording with a behavioural difference;
**Nit** = editorial.

---

## §0 Target questions

- **Major — T3's machine-count half has no defined answer.** T3 (`SPEC.md:34`)
  asks "how many recyclers are needed"; the only worked T3 document sizes from
  an amount (`SPEC.md:1425`), which by `SPEC.md:845-846` yields
  `MachineSeconds`, and `.machines` on a `MachineSeconds` process is an error
  (`SPEC.md:1054`). The sole conversion is `MachineSeconds(τ) / time`
  (`SPEC.md:858`) and no example uses it, so no target question is answered
  end-to-end by an example in Appendix B.

## §1 Design principles

- **Major — P3 vs §5.3's unconditional quantity errors.** P3 (`SPEC.md:56-61`)
  says an operator "errors only when nothing survives", i.e. a facet it cannot
  combine with is *dropped*. §5.3 instead declares errors that are not drops:
  two `per`s (`SPEC.md:712-713`), bundle×bundle (`SPEC.md:710`), division by a
  multi-term bundle (`SPEC.md:723-724`). Repro: `1:iron-plate:/s *
  2:copper-plate:/s` — P3 implies "drop `quantity`, error only because nothing
  survives"; §5.3:712-713 says error regardless. The same drop-vs-error gap appears
  in §5.5 ("require a nonnegative integer factor", `SPEC.md:868`).
- **Major — P4 vs §6.6.** P4 (`SPEC.md:62-65`) promises "Adding a facet adds
  rows to the operator matrix, not syntax", yet each facet contributes new
  *tokens* in §6.6 (`SPEC.md:1050-1082`): `.machines`, `.crafts`, `.power`,
  `.energy`, `.pollution`, `.productivity`, `.:speed:`, `.:time:`,
  `.:electric:`, `.:module-slots:`. §4.4's token inventory
  (`SPEC.md:529-550`) lists none of them; only the generic `.property` level
  exists. `quantity` alone needs no property, `machine` needs four.
- **Minor — P5 vs §6.3.** P5 says "Quality is never a distribution over
  outcomes" (`SPEC.md:66-72`); §6.3 says "Each product is split across those
  qualities, which is why `.out` reports one row per quality"
  (`SPEC.md:953-955`) with outcome percentages. Either P5 needs a
  process-level carve-out or §6.3 is unfounded.

## §2 Program model

- **Major — §2.2's "stored adjustment" has no representation.** "Assignment
  works on the row's total: `=` replaces what the recipes produce, and the
  stored adjustment is only the difference" (`SPEC.md:114-117`) requires a
  per-(row, resource) adjustment, but `Process.rows` is
  `BTreeMap<(MachineWithConfig, Recipe), f64>` — one scale per row, no
  per-resource field (`SPEC.md:972-975`). Repro: `p.:iron-plate: = 100` on a
  process whose row also outputs `:sludge:` — the only storable change (the row
  scale) silently rescales the co-product.
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

- **Major — levels 3/4 cannot be composed.** `.property` is level 3 and `.`
  with a resource operand is level 4 (`SPEC.md:453-455`), so postfix binds
  tighter: `v.:iron-plate:.abs` parses as `v . (:iron-plate:.abs)`, handing the
  `.` operator a quantity where §5.3:778 requires a resource — an error, though
  `(v.:iron-plate:).abs` is the only sensible reading. Same for
  `belt.:iron-plate:.rate`. No rule or example covers mixing levels 3 and 4,
  and `.abs` is explicitly a property (`SPEC.md:1039`).
- **Major — the justification for level 4 is void.** "binary `.` is looser than
  juxtaposition so that `:common: :iron-plate:` groups into a single selector
  operand" (`SPEC.md:468-470`) contradicts §4.5, where the pair is one token
  "during lexing, so no precedence rule and no lookahead are involved"
  (`SPEC.md:556-559`).
- **Major — unary `-` at level 7 makes `^` and filter right-hand sides
  unparseable.** Level 7 (`SPEC.md:458`) is looser than `^` (457) and the
  filters (456), so `2 ^ -3` and `p.net > -5` have no parse; the documented
  mirror case `-2 ^ 2` = `-(2 ^ 2)` (`SPEC.md:483-486`) works, but the
  right-hand cases are unmentioned and require parentheses.
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

- **Major — "the applied amount" is the whole rate→item conversion and is
  undefined.** `SPEC.md:636` is the only rule turning a `stack_rate` into item
  quantities. B.1's exact `540:iron-plate:/s` (`SPEC.md:1391`, "45 items/s …
  times 4 … times 3") requires `Σ rate[limit] × min(limit, stack_size) ×
  count`, which neither §5.2 nor §6.1 (`SPEC.md:879-890`) states. Relatedly,
  §6.1 calls the belt rate "`belt speed * 60 * 8` **items** per second"
  (`SPEC.md:883`) while `stack_rate` is "item **stacks** per second"
  (`SPEC.md:205`); the two coincide only at `Limit(1)`.
- **Major — "config that is one beacon and nothing else" is ambiguous.**
  `SPEC.md:637`. One *group* vs count 1 decides `:foundry:(2:beacon:)
  :speed-module-3:`: fill both beacons, or an error. Nothing says which.
- **Major — is an empty module list a module list?** The row "a module list | a
  module list | the merged module list" (`SPEC.md:638`) applies to the right
  operand of `:speed-module-2: 2 :beacon:` (whose config has an empty module
  list), which `SPEC.md:665-667` declares an error. Whether the empty list
  participates decides the outcome.
- **Major — non-integer config scaling: drop or error?** "`*` and `/` by a
  number scale the module and beacon counts, and require a nonnegative integer
  factor" (`SPEC.md:867-869`). P3 implies the `config` facet drops and the
  rest survives; §5.5 implies an error. Repro:
  `0.5 (:foundry: 2 :speed-module-3:)` — machine+quantity with a halved count
  and dropped config, or an error?
- **Minor — "the only common one" does not choose a quality.**
  `SPEC.md:853-856`. `2 :foundry: making :transport-belt:` yields five rows
  whose recipe differs only in quality (`SPEC.md:994-1000`), so "the only
  common one" is satisfied five times over; which row is kept (and thus which
  quality the machines are counted at) is unstated.
- **Minor — `-` and `.abs` are defined only for quantity and config.**
  `SPEC.md:742-745`. `T`, `K`, `M`, `P` are unstated (`-belt.rate`, `-p`).
- **Minor — `^` coverage and drop-vs-error.** `SPEC.md:731-740` never mentions
  `T`/`K`/`M` bases, and "A bundle base is an error" (`SPEC.md:739`) conflicts
  with A.3's "Every other pair drops" (`SPEC.md:1365`). `(2:iron-plate:)^0`
  and `(2:iron-plate:/s)^1` are undefined.
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

- **Major — `craft` is simultaneously an exponent and a row.** §3.1
  (`SPEC.md:201`) and §5.3 (`SPEC.md:673-675`) make `craft` an integer exponent
  of the quantity; §5.4:813 speaks of "a quantity of items, fluids, or crafts",
  §6.6:1055 returns "the craft row", and §7.7:1275-1276 prints "net rows other
  than `craft`". Nothing says what `.crafts` returns, whether `craft` can be
  selected (`p.:craft:`?) or filtered, or how a quantity can carry both a
  `craft` exponent and `craft` terms.
- **Major — the quality-module split numbers do not follow from the rule.**
  "Reaching `n` levels above the recipe's quality has probability `p^n`, and
  the recipe's own quality takes the remainder" (`SPEC.md:949-951`) with
  `p = 0.1` gives normal `0.9` (not `88.89%`) and exclusive shares
  `0.09/0.009/0.0009/0.0001`; the listed `10%/1%/0.1%/0.01%`
  (`SPEC.md:952-954`) are the cumulative `p^n` and double count the mass. The
  five shares only sum to 1 if normal is `88.89%`, which neither the rule nor
  `p` explains.
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
- **Minor — no behaviour is stated for `Unspecified` in §6.6 or §7.7.** Every
  row of the selector table (`SPEC.md:1050-1082`) and every process rendering
  rule (`SPEC.md:1275-1290`) presumes one of `Unscaled`/`Machines`/
  `MachineSeconds`; a process created by sizing with zero
  (`SPEC.md:828-832`) matches none of them (`.machines`, `.crafts`, `.power`,
  `.energy`, `.net`).

## §7 Display

- **Major — the exponent rendering names values that cannot exist, and omits
  the ones that can.** "A positive exponent other than one follows the term
  (`:iron-plate:²`)" (`SPEC.md:1205`) — resource terms have coefficients, not
  exponents (`SPEC.md:673-675`, D31 `SPEC.md:1489`), so no such quantity
  exists; and `²`/`₁` are not tokens (`SPEC.md:545-550`). Conversely
  `(2 s) ^ 2` = `4 s²` (`SPEC.md:736`) is a value that does exist and has no
  stated rendering, so the round-trip claim (`SPEC.md:1184-1186`) fails for it.
- **Major — machine count 1 round-trips to a different value.** "The count when
  it is not exactly one" (`SPEC.md:1216`) prints `1:foundry:` as `:foundry:`,
  but §6.3 makes absent-count distinct from count 1 (`SPEC.md:934-936`,
  Unscaled vs `Machines(1)` after `making`) and §3.5 makes machine equality
  exact (`SPEC.md:341-343`). Either round-trip fails or §6.3's distinction is
  empty.
- **Major — the approximate-zero threshold is never defined.** It is normative
  for canonicalization and display (`SPEC.md:345-347`, `SPEC.md:1153`) yet has
  no value, no unit and no setting. §7.2's own example `0.0001234 → 0.000123`
  (`SPEC.md:1162`) is only correct if the threshold is below `1.234e-4`;
  `1e-12` is stated to print as `0` (`SPEC.md:1153-1155`, no number).
- **Minor — an undefined row notation.** `288<foundry>` and
  `53.333s*<foundry>` (`SPEC.md:1284`) are defined nowhere and conflict
  with §7.5's machine syntax (`2:foundry:`). Also, "single-line result" is
  defined twice with different content: "the first facet present, then `…`"
  (`SPEC.md:1139`) and "the machine total, then the net rows"
  (`SPEC.md:1275`).

## §8 Diagnostics

- **Major — no rule maps a failure to one of the four kinds.** §8 lists four
  kinds with one example each (`SPEC.md:1313-1319`), but every other "is an
  error" in the document leaves the kind and message open: config promotion
  (`SPEC.md:282`), `:iron-plate: q` (`SPEC.md:262`), `^` on a bundle
  (`SPEC.md:739`), non-integer config scale (`SPEC.md:869`), `.productivity`
  with disagreeing rows (`SPEC.md:1057`), `Limit(0)` (`SPEC.md:880`), beacon
  profile length (`SPEC.md:921-923`). A conforming implementation cannot be
  distinguished from a non-conforming one, and Appendix B asserts no error
  behaviour at all.

## Appendix A — operator matrix

- **Minor — "drops" vs "is an error" is never reconciled in A.** A.3 "Every
  other pair drops" (`SPEC.md:1365`) vs §5.3's "A bundle base is an error"
  (`SPEC.md:739`); A.1's `C, C` "`-` drops the config" (`SPEC.md:1348`) vs §8's
  facet-loss error (`SPEC.md:1318`). Whether a drop inside a multi-facet value
  is silent (P3) or fatal is the same unresolved question as P3 versus §5.3's
  explicit arithmetic errors.

## Appendix B — acceptance examples

- **Blocker — B.4's `a == b` has no semantics.** `SPEC.md:1449`. `==` exists
  only as a quantity filter whose result is a filtered quantity, not a boolean
  (`SPEC.md:747-752`, `SPEC.md:761-775`); §3.5 defines structural equality
  (`SPEC.md:334-347`) but gives it no operator, and no rule implements `==` for
  machines, processes, configs or qualities. Repro: `2:iron-plate: == 2`
  yields `2:iron-plate:`, so `a == b` cannot mean "equal".
- **Major — B.3 (and §2.6) may fail config validation.** `R = :recycler:
  (4 :quality-module-2:)` (`SPEC.md:1423`; also `SPEC.md:175`) puts four
  quality modules in a recycler, and §5.2 requires "module slot counts"
  validation on every machine-config juxtaposition (`SPEC.md:659-661`). If the
  recycler has three module slots in the game data, both documents fail at
  parse-to-value time. Not checkable in this repo (no registry dump).
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

## Status / open markers

- **Major — `[open]` is defined but never used, while several passages are
  plainly undecided.** The header states that "[open]" marks undecided
  sections (`SPEC.md:3-5`); a search finds zero occurrences. Undecided content
  is instead hidden in prose: §3.4's MVP caveat (`SPEC.md:331-333`), the
  operator/facet rows missing from §5 and Appendix A (see the coverage table
  below), the approximate-zero threshold (`SPEC.md:345`), and "the only common
  one" (`SPEC.md:854`). No `…` stub lines remain: the four `…`
  occurrences (`SPEC.md:111`, `1470`, `1477`, `1481`) are ordinary ellipses
  inside prose.

---

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
| `/` | ✓ | ↓ | ✓ | ✓ | ✓/? | ↓ | ↓ | ✓ | ✗ | 1351-1360 |
| `^` | ✓ | ↓ | ↓ | ↓ | ↓ | ↓ | ↓ | ? | ✗ | 1363-1365 |
| filter, quality RHS | ✓ | ? | ? | ? | ? | ? | ? | ? | ✗ | 613, 761-775 |
| filter, number RHS | ✓ | ? | ? | ? | ? | ↓ | ↓ | ? | ✗ | 758-763 |
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
| process properties | · | · | · | · | · | · | · | ✓/? | · | 1050-1082 |
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
- **Every process property on `Unspecified`**: see §6 finding.

### Conflict cells the table cannot express

| Cell | Conflict |
|---|---|
| `+`/`-`/`*` with `Unspecified` | `SPEC.md:831-832` (identity) vs `SPEC.md:850-851` (same extent kind required) |
| `C` under `/` | `SPEC.md:1360` ("nonnegative integer factors only") — `3 modules / 2` is neither an error nor a drop by any stated rule |
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

- Whether a recycler has four module slots (`SPEC.md:175`, `SPEC.md:1423`). The
  repository contains no dump data (no `*.json` dump anywhere; only loader code
  under `crates/hactorio-data/src`), so the slot-count invalidation of both
  documents is [UNVERIFIED].
- `SPEC.md:883`'s belt constant (`belt speed * 60 * 8` = 45 items/s) and
  `SPEC.md:892-896`'s inserter capacities are game-data claims; only their
  internal consistency with B.1's arithmetic was checked here.
- Whether the registry's quality levels are `0,1,2,3,5` (`SPEC.md:583`) — not
  checkable without the dump; the `.multiplier` formula
  (`SPEC.md:405`) is consistent with those levels.
