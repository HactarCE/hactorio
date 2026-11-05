# `SPEC_corrected.md` — internal validation

Status: the findings from the first pass are resolved. This document lists only
what is still open, with citations into `SPEC_corrected.md`.

Severity: **Major** = a user or implementer hits the ambiguity; **Minor** =
missing detail or loose wording with a behavioural difference; **Nit** =
editorial.

## §1 Design principles

- **Minor — P5 vs §6.3.** P5 says "Quality is never a distribution over
  outcomes" (line 62); §6.3 splits each product across qualities (line 982).
  Either P5 needs a process-level carve-out or §6.3 contradicts it.

## §2 Program model

- **Minor — compound-assignment operand types.** `*=` and `/=` take a
  dimensionless number (line 103), but what `+=` and `-=` accept is unstated,
  and so is assignment to an `Unscaled` or `Unspecified` process (lines 1015,
  1019).
- **Minor — no rule for malformed blocks.** §2.1 has no rule for a `}` without
  a header, an unclosed `{`, or a parse error in the `loop` header (line 91).
- **Nit — comment vs token.** "`#` outside a `:token:`" (lines 85, 551) implies
  `#` may occur inside a token, and a trailing comment after `{` conflicts with
  "`{` must be the last token on its line" (line 91).

## §3 Value model

- **Minor — quality × quality is undefined.** "A value with no item terms is
  unchanged" (line 287) leaves `:rare: :epic:` and two stored qualities
  juxtaposed with no stated result.

## §4 Grammar

- **Minor — `.5` vs the `.` operator.** `.5` is a number (line 500) and `.` is
  punctuation, so `v.5` has no stated tie-break; it is read as `v` scaled by
  `0.5`.
- **Minor — `1MJ` has two readings.** The scale suffix `M` (line 502) and the
  unit `MJ` (line 515) both match; the values coincide but which rule wins is
  unstated.
- **Minor — signal selectors are not named in the `.` row.** The precedence
  table's `.` row covers "a property or a resource operand" (line 448), while
  §6.6 also has signal selectors after `.` (line 1082).
- **Minor — filters bind tighter than arithmetic without a stated reason.**
  Quality filters sit at level 4 (line 449), above every arithmetic operator, so
  `q + r < :rare:` filters `r` only. The other two inversions are justified;
  this one is not.
- **Nit — chained filters.** Left-associative filters accept `a < b < c`
  (lines 449, 774), which no rule mentions.

## §5 Operator semantics

- **Minor — "the only common one" does not choose a quality.** A process of
  quality variants has five rows whose recipe differs only in quality (line
  877); which row is kept, and therefore which quality the machines are counted
  at, is unstated.
- **Minor — `-` and `.abs` are defined only for quantity and config.** `T`,
  `K`, `M`, and `P` are unstated (line 770).
- **Minor — self-cancelling division.** `3:iron-plate:/:iron-plate:` is
  unstated: cancel, move to `per`, or error (line 741).
- **Minor — §5.1's table misuses its own columns.** The quality-operand row
  lists magnitude facets under "Reads" (line 614) in a table about reference
  facets, and `.` is said to read the `resource` facet (line 617) while §5.3
  selects quantity terms and process rows (lines 774, 798).
- **Minor — filters and `.` selection on `T`, `K`, `M`, `C`.** Both are defined
  for quantities and processes only (lines 774, 798).
- **Minor — `.:item:` / `.:fluid:` on `M` and `C`.** The table's operand set is
  "quantity or process" (line 1115).
- **Minor — `.:stack-size:` on a process, or where the reference names a
  fluid.** The row says "naming an item" (line 1114); the fluid case is
  unstated.
- **Minor — `:stacked-belt:` with no stack facet.** It requires every stack
  facet to be at `Limit(1)` (line 909); a value with none has no stated result.
- **Minor — `:recycling:` on a scaled or non-item value.** It reads `resource`
  (line 618), which scaling destroys, so the result is unstated.
- **Minor — `in`/`into`/`from` with other extents on the right.** Only an
  `Unscaled` process or a machine is listed (line 834); `Machines`,
  `MachineSeconds`, and `Unspecified` are neither allowed nor rejected.
- **Minor — juxtaposition with a process.** §3.4 says a process never coexists
  with another value kind (line 306), but §5.2 has no row for `p q` (line 649).

## §6 Facets in depth

- **Minor — empty config rendering.** An empty config "renders as `0` only when
  it is the whole value" (line 943), but §7.5's config grammar has no empty
  production and no `0`.
- **Minor — undefined terms in §6.6.** "modifier power" (line 1101) is defined
  nowhere.
- **Minor — duplicated selector rows.** `.:item:` and `.:fluid:` appear twice
  with different wording (lines 1115, 1121).

## §7 Display

- **Minor — an undefined row notation.** `288<foundry>` and
  `53.333s*<foundry>` (line 1327) are defined nowhere and conflict with §7.5's
  machine syntax.

## Appendix B — acceptance examples

- **Minor — B.1's exact number depends on a setting.** `540:iron-plate:/s`
  (line 1450) requires the belt-stacking level to be 4 (line 907), while
  Appendix B frames its exact values as registry constants.
- **Nit — two T1 assertions are not invariants.** "`p.machines` is a machine
  count" and "`p.:electric:` # watts" (lines 1446-1447) assert no property, so
  they cannot fail.

## Appendix C — decision log

- **Minor — D20's "unobservable" claim is false.** The `-`-tighter-than-`+`
  inversion is called unobservable (line 1544); with `f64` coefficients the
  grouping changes the bit pattern, and the `1e-9` threshold decides whether
  that is visible.
- **Minor — D44's "naming them (`:recycling:`)" is inconsistent with §4.4.**
  `:recycling:` is a prefix operator that reads a `resource` facet (line 618),
  so it cannot name a hidden recipe on its own (line 1568).
- **Minor — the log omits decisions that are normative elsewhere** (line 1521
  onward): the filter-vs-arithmetic precedence inversion, `Unspecified`, the
  assignable selector, and the `1e-9` threshold.
