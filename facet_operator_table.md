# Facet operator table

Source: `2026-10-22_design_doc_2.md` (§3, §3.1–§3.8, §4.1–§4.8, §10). Property
accessors (`.` and every named/signal selector) are intentionally excluded.

## Legend and reachability

Facets: `I` item, `R` recipe, `M` machine, `P` process, `C` config, `B` belt,
`T` rate, `K` capacity.

Matrix axes split the item facet by dimension, because nearly every operator
treats the two forms differently:

- `S` — **dimensionless scalar**: `item` present with an empty monomial (§3.1.1).
- `U` — **unit-bearing item**: any non-empty monomial: pure item/fluid quantity,
  rate quantity (`item/s`, `item·s⁻¹`), time (`second`), energy (`joule`),
  `craft`, or a multi-unit product.

`B` never occurs without `T` (§3.1.1), so a matrix row/column `B` denotes the
value `B+T`. Rows/columns `M`, `C`, `T`, `K`, `R` denote that facet alone (no
item). A cell holds the surviving facet set; `err` means no facet survives (the
operation errors); `err!` means an explicit error is mandated by §10 or by a
stated validation gate; `unreachable` means §3.1.1 never constructs the facet
set, so the design defines no semantics and none are invented.

Facet sets that §3.1.1/§4 do construct inside one value, with constructors:

| Set | Constructed by |
|---|---|
| `I` | item/fluid token, quantity literal, arithmetic result |
| `I+R` | item token whose entry has a same-id recipe |
| `I+M`, `I+M+R` | machine item token (with/without a same-id recipe) |
| `M` | `with` result; machine arithmetic on machine-only values |
| `P` | `making`, `in` |
| `I+C`, `I+C+R` | module/beacon token (with a same-id recipe) |
| `I+B+T`, `I+B+T+R` | belt token |
| `B+T` | `+` of two different belt tiers |
| `I+T`, `I+T+R` | inserter/pump token (with a same-id recipe) |
| `I+K`, `I+K+R` | container token (chest, car, cargo wagon, tank, fluid wagon) |
| `T`, `K` | `.rate`, `.capacity`, `K / time`, `:stacked-belt:` |
| `C` | config addition/scaling |
| `R` | recipe-only token |

Never constructed as one value: `P` with any other facet; `M` with a separate
`C`; `T` with `K`; `I+T+K`; `B` without `T`; `R` with `P`. Recipe may coexist
with `I` and at most one of `M`, `C`, `T`, `K`, or `B+T`; it never coexists with
`P`, or with those facets without `I` (except the recipe-only value `R`). Those
sets are `unreachable` as a single value and no semantics are invented for them.
Matrix cells are never `unreachable`: the two operands are independent values,
and every listed single facet is constructible on its own (a matrix `B` is the
reachable pair `B+T`).

The recipe-bearing variants `I+R`, `I+M+R`, `I+C+R`, `I+T+R`, `I+K+R`, and
`I+B+T+R` otherwise behave like their base sets: every operator except
`making` (which consumes the RHS recipe) drops `R` and leaves the base result.

## `+`

`+` combines like facets only (§3.1.1); there is **no** scaling exception, so a
scalar does not penetrate any other facet.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `I` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **U** | `err` | `I` (equal units) / `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **R** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **M** | `err` | `err` | `err` | `M` (same machine and config) / `err` | `err` | `err` | `err` | `err` | `err` |
| **P** | `err` | `err` | `err` | `err` | `P` (both solved, same extent kind) / `err!` | `err` | `err` | `err` | `err` |
| **C** | `err` | `err` | `err` | `err` | `err` | `C` | `err` | `err` | `err` |
| **B** | `err` | `err` | `err` | `err` | `err` | `err` | `B+T` (same cap) / `T` (cap differs) | `T` (same medium) / `err` | `err` |
| **T** | `err` | `err` | `err` | `err` | `err` | `err` | `T` (same medium) / `err` | `T` (same medium) / `err` | `err` |
| **K** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `K` (same medium) / `err` |

The table is symmetric in facet survival and in the merged values.

### Combination rows (`+`)

| LHS set | RHS set | Result | Condition |
|---|---|---|---|
| `I+R` | `I+R` | `I` | equal item units; recipe always drops |
| `I+M` / `I+M+R` | `I+M` / `I+M+R` | `I+M` | same machine and merged config; counts combine |
| `I+M` | `I+M` | `I` | differing machine or config → machine drops, item survives |
| `I+C` | `I+C` | `I+C` | equal item units; configs merge |
| `I+C` | `I+C` | `C` | different item units → item drops, config still merges |
| `I+B+T` | `I+B+T` | `I+B+T` | same belt cap and same item unit; counts and rate terms add |
| `I+B+T` | `I+B+T` | `B+T` | different belt tier → item unit differs, belt/rate survive |
| `I+B+T` | `I+B+T` | `I+T` | different cap (stacked vs unstacked) → belt drops, rate merges |
| `I+B+T` | `T` | `T` | belt and item are one-sided, rate merges by medium/cap |
| `I+T` | `I+T` | `I+T` | inserter tokens: item units equal, rate terms merge by cap |
| `I+T` | `I+T` | `T` | different item units → item drops, rate still merges by cap |
| `I+K` | `I+K` | `I+K` | same item unit, same medium; capacities merge |
| `I+K` | `I+K` | `K` | different item units → item drops, capacities still merge |
| `I` (plain item rate) | `T` | `err` | item-facet rates never mix with a rate facet |
| `C` | `I` | `err` | config only merges with config |

### Special conditions (`+`)

- **No scaling:** `2 + :foundry:` is `err`; only `*` and `/` scale.
- **Item:** strict unit algebra. Equal monomials only: `2 + 3` (both empty
  monomials), `2 s + 3 s`, `craft + craft`. Scalar plus unit-bearing → `err`;
  `2 :iron-plate: + 3 :copper-plate:` → `err`; `craft` never converts to items;
  time combines only with time; multi-unit monomials require identical
  exponents for every base unit.
- **Recipe:** always dropped, including when both operands carry one. Only
  `making` consumes a recipe facet.
- **Machine:** requires the same prototype **and** the same merged config;
  counts combine with `None` read as one and the result has a concrete count.
  A differing machine/config drops the machine facet (no error by itself).
- **Process:** both operands must be **solved** and of the same `Extent` kind.
  An unsolved operand or mixed `Machines`/`MachineSeconds` is an explicit error,
  not a drop. `net`, `power_w`/`energy_j`, and the per-machine-spec counts or
  machine-seconds add; zero and negative values are legal.
- **Config:** counts are nonnegative integers by construction. `+` adds module
  counts by id and beacon-group counts for the same beacon+module set; groups
  with different module sets stay separate; zero terms are elided and an
  all-zero config stays present but empty. `+` never modifies a beacon group and
  never attaches modules to one; only `Cb Cm` juxtaposition attaches.
- **Belt:** survives only with equal `Cap`; the cap is retained and counts
  add. Tiers need not match, so `(:transport-belt: +
  :fast-transport-belt:)` keeps `B+T` and loses `I`. If the caps differ the belt
  facet drops and the rate still merges.
- **Rate/capacity:** terms merge by cap and cap sets need not match; item medium
  must match item medium, fluid must match fluid, and mixed media drop the
  facet (a drop, not an error). Zero terms are removed while the facet stays
  present, so a zeroed rate is `0/s`, not an absent facet.
- **Item-facet rates** (`3 :iron-plate:/s`) are not rate facets; they only add
  as item units and error against a `T`.

## `-`

`-` negates quantitative facets and drops the same facets as `+`, with one
changed cell: config minus config has no result (§3.6: config counts are not
signed). Recipes never subtract.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `I` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **U** | `err` | `I` (equal units) / `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **R** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **M** | `err` | `err` | `err` | `M` (same machine and config) / `err` | `err` | `err` | `err` | `err` | `err` |
| **P** | `err` | `err` | `err` | `err` | `P` (both solved, same extent kind) / `err!` | `err` | `err` | `err` | `err` |
| **C** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **B** | `err` | `err` | `err` | `err` | `err` | `err` | `B+T` (same cap) / `T` (cap differs) | `T` (same medium) / `err` | `err` |
| **T** | `err` | `err` | `err` | `err` | `err` | `err` | `T` (same medium) / `err` | `T` (same medium) / `err` | `err` |
| **K** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `K` (same medium) / `err` |

### Combination rows (`-`)

Every `+` combination row carries over with subtraction of the values, except
the config rows:

| LHS set | RHS set | Result | Condition |
|---|---|---|---|
| `I+C` | `I+C` | `I` | equal item units; config drops |
| `I+C` | `I+C` | `err` | different item units: item and config both drop |
| `C` | `C` | `err` | config drops, nothing remains |
| `I+M` | `I+M` | `I+M` / `I` / `err` | same machine and config; same machine with different config → machine drops, item remains; different machine → both drop |
| `P` (solved) | `P` (solved) | `P` | same extent kind |

### Special conditions (`-`)

- **No scaling:** a scalar subtracts only from a scalar.
- **Values negate:** item coefficient, machine count (`a - b`, `None` as one,
  concrete result), process extent/`net`/`power_w`/`energy_j`, belt count, rate
  amounts, capacity amounts.
- **Recipe:** always drops; a recipe never subtracts.
- **Process:** both operands must be solved and of the same extent kind;
  otherwise `err!`. Zero and negative extents are legal.
- **Config:** drops the config facet entirely (counts are not signed), so
  `C - C` is `err` and `I+C - I+C` keeps only the item facet.
- **Belt:** same cap required; the cap is retained; counts subtract.
- **Rate/capacity:** terms merge by cap within the same medium; `r - r` is a
  zero rate/capacity with the facet **present** (`0/s`), not an absent facet.
  Mixed media drop the facet.
- **Item-facet rates** still do not mix with a rate facet.

## `*`

Only `*` and `/` apply the dimensionless-scalar exception (§3.1): `S` scales
every scalable facet of the other operand without carrying it. Everything else
is per-facet.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `I` | `I` | `err` | `M` (count `Some(s)`) | `P` (scales; unsolved → `Machines(s)`) | `C` (nonnegative integer) / `err` | `B+T` (count and amounts scaled) | `T` | `K` |
| **U** | `I` | `I` (monomial product) | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **R** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **M** | `M` (count `Some(n·s)`; `None` read as 1) | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **P** | `P` | `P` (`MachineSeconds`) if `U` is time, else `err!` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **C** | `C` (nonnegative integer) / `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **B** | `B+T` (count and amounts scaled) | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **T** | `T` | `K` (time) / `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **K** | `K` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |

### Combination rows (`*`)

| LHS set | RHS | Result | Condition |
|---|---|---|---|
| `I+M` / `I+M+R` | `S` | `I+M` | count `Some(n·s)`, `None`→1; recipe drops |
| `I+M` | `U` | `I` | monomial product; machine drops (unit-bearing multiplier) |
| `I+R` | `S` | `I` | recipe drops via scaling |
| `I+C` | `S` | `I+C` / `I` | config scales only for a nonnegative integer, else drops |
| `I+C` | `U` | `I` | config drops on any unit-bearing multiplier |
| `I+B+T` / `I+B+T+R` | `S` | `I+B+T` | item, belt count, and rate amounts scale; cap fixed; recipe drops |
| `I+B+T` | `U` | `I` | monomial product; belt and rate drop |
| `I+T` | `S` | `I+T` | item coefficient and rate amounts scale |
| `I+T` | `U` (time) | `I` | non-isolated rate drops; item inherits `second` (`3 :iron-plate:/s * 8 s` = 24 iron-plate) |
| `I+T` | `U` (other units) | `I` | rate drops, monomials multiply |
| `I+K` | `S` | `I+K` | capacity amounts scale |
| `I+K` | `U` (anything) | `I` | capacity × time is undefined; capacity drops |
| `T` (isolated) | time | `K` | only isolated rate converts; medium and caps kept |
| `P` (unsolved) | `S` | `P` (`Machines(s)`) | scaling, including `1`, solves the process |
| `P` (`Machines(n)`) | time | `P` (`MachineSeconds(n·t)`) | `net` scaled, `energy_j = power_w·t`, `power_w` cleared |
| `C` | `S` with fractional/negative `s` | `err` | `0.5 * :speed-module-3: / s` keeps the item rate, no config |
| `I+T` | `I+T` | `I` | rate × rate undefined → rate drops; item monomials multiply |
| `I+K` | `I+K` | `I` | capacity × capacity undefined → capacity drops |
| `I+B+T` | `I+B+T` | `I` | belt × belt and rate × rate undefined; item product (`b1 * b2`) |
| `I+C` | `I+C` | `I` | config × config drops config; item product |

### Special conditions (`*`)

- **Dimensionless scalar:** scales item coefficient, belt count, rate amounts,
  capacity amounts, machine count, and (nonnegative integer only) config counts.
  It never adds a facet the other operand lacks.
- **Machine:** only the count scales; per-machine modules/beacons are fixed.
  `Some(n)` becomes `Some(n·s)`; `None` is read as one and attached as
  `Some(s)` — even for `s = 1` (§3.4). Unit-bearing multiplication drops the
  machine facet (e.g. `(:foundry: with sm) * 2 s` drops it and leaves no facet →
  `err`).
- **Process:** scalar scaling scales a solved process; an unsolved process is
  solved as `Machines(s)`; `* 1` therefore solves it. `Machines(n) * time` →
  `MachineSeconds(n·t)` with `net` scaled and `energy_j = power_w·t`. An
  unsolved process × time assumes one machine. `Machines * 2 J`,
  `Machines * 2 second²`, and `MachineSeconds * time` are explicit errors.
- **Config:** a scalar must be finite, nonnegative, and integral; `1.0` counts.
  Fractional or negative → config facet drops (no error). Module counts inside a
  beacon group are per-beacon and never scale. Multiplying by a unit-bearing
  value, by time, or by another config drops config. `0` zeroes all counts, and
  the config stays present but empty.
- **Belt:** a scalar keeps the belt facet and scales its count; the cap is
  unchanged; the belt's rate amounts scale with it. Time arithmetic and any
  unit-bearing multiplier drop the belt facet.
- **Rate/capacity:** scalar scales every amount. `rate * time → capacity` only
  for isolated values; a non-isolated rate (belt token, `I+T`) drops the rate
  facet instead.
- **Recipe:** always drops (any operation except `making`).
- **Two rate, config, belt, or capacity facets:** `*` never merges them; those
  facets drop, and only item monomials multiply (`b1 * b2` is an item product of
  both belt items). With no item facet on either side the drop leaves `err`.

## `/`

Division uses the same dimensionless-scalar exception when the **divisor** is a
scalar. Isolated rate/capacity division has its own rules (§3.8) and errors
rather than drops on media/cap mismatch.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `I` (0 divisor → `inf`/`NaN`) | `I` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **U** | `I` | `I` (monomial quotient) | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **R** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **M** | `M` (count `/s`; `None` → `Some(1/s)`) | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **P** | `P` (solved scaled; unsolved → `Machines(1/s)`) | `P` → `Machines` if LHS is `MachineSeconds` and `U` is time; `Machines / time` and unsolved `/ time` → `err!` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **C** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **B** | `B+T` (count and amounts divided) | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **T** | `T` | `err` (rate ÷ time undefined) | `err` | `err` | `err` | `err` | `err` | `I` (dimensionless) / `err!` | `I` (`s⁻¹`) / `err!` |
| **K** | `K` | `T` (time) / `err` | `err` | `err` | `err` | `err` | `err` | `I` (`s`) / `err!` | `I` (isolated) / `err!` |

### Combination rows (`/`)

| LHS set | RHS | Result | Condition |
|---|---|---|---|
| `I+M` / `I+M+R` | `S` | `I+M` | count divided; recipe drops |
| `I+B+T` | `S` | `I+B+T` | belt count and rate amounts divided; cap fixed |
| `I+B+T` | `U` (time) | `I` | belt and rate drop; monomial keeps `second⁻¹` from the item facet |
| `I+T` | `U` (time) | `I` (`s⁻²`) | rate facet is not isolated → drops |
| `I+K` | `U` (time) | `I` | capacity is not isolated → drops |
| `K` (isolated) | time | `T` | the only capacity→rate conversion; medium and caps kept |
| `K` (isolated) | `T` | `I` (`s`) | same medium; single same cap for items |
| `T` (isolated) | `K` | `I` (`s⁻¹`) | same medium; single same cap for items |
| `T`,`K` (isolated, same kind) | same kind | `I` | dimensionless, same medium, single same cap |
| `I+C` | `S` | `I` | config facet drops; item divides |
| `I+T` / `I+K` / `I+B+T` / `I+C` | same set | `I` | non-isolated facets drop; monoterm item quotient (dimensionless when units cancel) |

### Special conditions (`/`)

- **Config divisor or dividend:** a config facet is dropped by `/`, and the
  other facets continue. A config-only division errors because no facet
  survives. Configs are scaled by `*` only; they are never divided.
- **Scalar divisor:** scales item, belt count, rate/capacity amounts, and
  machine count. Item/rate/capacity division by zero follows scalar semantics
  (`inf`/`NaN`).
- **Machine:** only counts divide; config stays fixed. An uncounted machine
  becomes `Some(1/s)`; a counted machine becomes `Some(n/s)`. Unit-bearing
  divisors drop the machine facet.
- **Process:** solved process ÷ scalar scales; unsolved ÷ scalar becomes
  `Machines(1/s)`. `MachineSeconds / time → Machines` (net totals ÷ time,
  `power_w = energy_j / time`, `n = tau / time`). `Machines / time` and unsolved
  `/ time` are explicit errors; `MachineSeconds / 0 s` yields `inf`/`NaN`.
- **Config:** division drops the config facet; it errors only when no other
  facet survives.
- **Belt:** scalar divisor keeps `B+T`; a time or unit-bearing divisor drops
  both.
- **Isolated rate/capacity division:** item values must have exactly one term
  and the same cap; fluids only need the same medium. Multi-term values,
  mismatched caps, and mismatched media are **errors**, not drops. An empty
  divisor map is a zero rate/capacity: a single-term dividend yields
  `inf`/`NaN` and needs no cap match; a multi-term dividend still errors.
- **Recipe:** always drops.
- Plain item rates/capacities are item-facet monomials; dividing them divides
  monomial exponents and never touches a rate/capacity facet.

## `^`

`^` applies only to item/scalar facets; every other facet drops. Powers require
integer exponents when the base monomial carries units.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `I` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **U** | `I` (integer exponent) / `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **R** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **M** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **P** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **C** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **B** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **T** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **K** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |

### Combination rows (`^`)

| Base set | Exponent | Result |
|---|---|---|
| `I+M` / `I+M+R` | `S` (integer) | `I` — machine and recipe drop |
| `I+B+T` / `I+B+T+R` | `S` (integer) | `I` — belt, rate, recipe drop |
| `I+C` | `S` (integer) | `I` — config drops |
| `I+R` | `S` (integer) | `I` — recipe drops |
| `I+T` / `I+K` | `S` (integer) | `I` — rate/capacity drop |
| `P`, `M`, `C`, `B+T`, `T`, `K`, `R` alone | any | `err` — no item facet survives |

### Special conditions (`^`)

- The result multiplies every exponent of the base monomial by the exponent;
  `craft`, `second`, `joule`, and prototype units all raise.
- Exponent must be dimensionless; a unit-bearing exponent (`U`) drops the item
  facet and errors.
- `^` is right-associative. Unary `-` binds looser: `-2 ^ 2` = `-(2^2)`.
  `:stack-size:`/`:stacked-belt:` bind tighter: `:stacked-belt: b1 ^ 2` =
  `(:stacked-belt: b1) ^ 2`.
- A dimensionless scalar base and exponent use raw `f64::powf`; there are no
  `0^0`, negative-base, or negative-exponent special cases. A unit-bearing base
  requires an exact integer exponent. A unit-bearing exponent errors; other
  exponent facets are ignored.

## Juxtaposition

Dispatch is ordered (§4.2, rules 1–5); the first matching rule wins, so rule 1
beats the rule-5 error.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `I` | `I` (rule 4: scales all scalable facets; recipe drops) | `err` | `M` (rule 4 count) | `P` (rule 4) | `C` (rule 4, nonnegative integer) / `err` | `B+T` (rule 4) | `T` (rule 4) | `K` (rule 4) |
| **U** | `I` | `err` (rule 5: two unit-bearing values need `*`) | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **R** | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **M** | `M` (rule 4 count) | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **P** | `P` (rule 4) | `err` | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **C** | `C` (rule 4, nonnegative integer) / `err` | `err` | `err` | `err` | `err` | `partial` (rule 3) | `err` | `err` | `err` |
| **B** | `B+T` (rule 4) | `I` (rule 1 rate application) | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **T** | `T` (rule 4) | `I` (rule 1) | `err` | `err` | `err` | `err` | `err` | `err` | `err` |
| **K** | `K` (rule 4) | `I` (rule 2) | `err` | `err` | `err` | `err` | `err` | `err` | `err` |

### Combination rows (juxtaposition)

| LHS set | RHS set | Rule | Result |
|---|---|---|---|
| `I+B+T` | `I` (pure item/fluid) | 1 | `I` — applied amount; belt and rate drop |
| `T` (isolated) | `I` (pure item/fluid) | 1 | `I` — applied amount |
| `I+K` | `I` (pure item/fluid) | 2 | `I` — applied amount; capacity drops |
| `K` (isolated) | `I` (pure item/fluid) | 2 | `I` — applied amount |
| `Cb` (one beacon group) | `Cm` (modules only) | 3 | `Cb` with modules attached; RHS item consumed, LHS item retained |
| `Cm` | `Cm` | 3 | module lists combine; both item facets consumed → `Cm` |
| `Cb` | `Cb` / `Cm` | 3 | `err!` — partial merge; only `Cb Cm` is defined |
| `C` with >1 beacon group | `Cm` | 3 | `err!` — only a single beacon group can be extended |
| `I+Cm` | `I+Cm` | 3 | plain module merge consumes both item facets → `Cm` |
| any `S` | any set | 4 | scales every scalable facet: item, belt count, rate/capacity amounts, machine count, integer config; recipe drops |
| any set | any `S` | 4 | same, operand order irrelevant |
| `I+M` / `I+M+R` | `I` | 5 | `err` — two item-bearing values need `*` |
| `I+B+T` | `I` | 1 | rule 1 wins over the multiplication error |
| `I+T` | `U` (rate quantity) | 5 | `err` — two unit-bearing values need `*` |
| `I` | `I` | 5 | `err` — two item-bearing values need `*` |
| `I` | `T` / `K` | 5 | `err` — an isolated rate/capacity must be the left operand |
| `M` (machine-only) | anything non-scalar | 5 | `err` — `with` removed the item facet |
| `P`, `R` | anything non-scalar | 5 | `err` |

### Special conditions (juxtaposition)

- **Rules 1–2** require the RHS to be a *pure* item/fluid quantity: one item or
  fluid unit at exponent 1 and no other units. A rate quantity (`…/s`) or a time
  value does not qualify and falls through to the rule-5 error. Application evaluates
  `rhs_coef · Σ amount · min(cap, stack_size)` (`Full` = the item's stack size;
  fluid uses `rhs_coef · volume rate`). All RHS facets except the item/fluid are
  dropped; an item-vs-fluid medium mismatch is an error. Belt application uses
  the rate facet and always drops the belt facet.
- **Rule 3:** partial. `Cb Cm`: the LHS has exactly one beacon group and the RHS
  has only modules; the modules attach to that group, the RHS item is consumed,
  and the LHS item is retained. `Cm Cm`: module lists combine and both item
  facets are consumed, yielding `Cm`. `Cb Cb`, `Cm Cb`, and a LHS with more than
  one beacon group are errors. Thus `bc sm` attaches but `sm bc` errors. The
  operation is associative where defined; `Cm Cm` is commutative and `Cb Cm`
  is not.
- **Rule 4** is exactly quantity-literal scaling: machine count becomes
  `Some(n)` (even `n = 1`; `None` read as one), item/belt/rate/capacity
  coefficients scale, config counts scale only for a nonnegative integer, and
  the recipe facet drops. `:foundry: 2` and `2 :foundry:` both give two
  machines.
- **Rule 5** is the dispatch error: two unit-bearing values, including two
  item-bearing values or two time values, require explicit `*`. Juxtaposition
  multiplication itself is only rule 4 (one dimensionless scalar). Examples:
  `2 :iron-plate: 3 :iron-plate:`, `2 :iron-plate: 3 s`, and `2 s 3 s` all
  error; `2 :iron-plate: * 3 :iron-plate:` is explicit multiplication.
- Precedence: juxtaposition binds tighter than `^` and `*`; `:stack-size:` and
  `:stacked-belt:` bind tighter than juxtaposition (`:stacked-belt: b1
  :iron-plate:` = `(:stacked-belt: b1) :iron-plate:`).
- Plain item rates are item facets. They do not implicitly multiply with another
  unit-bearing value; `3 :iron-plate:/s * 2 s` needs `*`.

## `with`

Accepts only a machine LHS and a config RHS (§4.4). Every other ordered pair is
an explicit error: a missing machine facet or a missing config facet is listed
in §10. The result is machine-only: item, recipe, and every other non-machine
facet drop, while the optional count is retained.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **U** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **R** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **M** | `err!` | `err!` | `err!` | `err!` | `err!` | `M` | `err!` | `err!` | `err!` |
| **P** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **C** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **B** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **T** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **K** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |

### Combination rows (`with`)

| LHS set | RHS set | Result | Condition |
|---|---|---|---|
| `I+M`, `I+M+R` | `C` | `M` | item and recipe drop; count retained |
| `M` | `I+C` | `M` | the RHS item is irrelevant; config merges |
| `P` | `C` | `err!` | processes are not machines; `with` on a process is an error |
| `I` (machine-less item) | `C` | `err!` | no machine facet |
| `M` | `S`, `T`, `K`, `B` | `err!` | no config facet |

### Special conditions (`with`)

- Config counts are per machine; merged modules/beacons are stored individually
  on the machine so later `with` calls can add more.
- `with` rejects invalid machine module slots/effects, invalid beacon groups,
  and over-capacity configurations. Beacon-slot validation happens when the
  group is constructed (juxtaposition), machine-slot validation here.
- The machine's optional count is kept unchanged; `with` never attaches a count.
- The result has no item facet, so a configured machine juxtaposed with an item
  is a dispatch error; use `making` instead.
- `with` is left-associative and repeatable: `m with sm with bc` =
  `(m with sm) with bc`.
- The RHS is a config expression (juxtaposition, `.`, `^`, unary, `*`, `/`,
  `+`, `-`, parentheses) that stops at `making`, `with`, `in`, `)`, `#`, so
  `m with sm + bc` = `m with (sm + bc)` and `m with sm making r` =
  `(m with sm) making r`.

## `making`

Requires a machine LHS and a recipe-bearing RHS (§4.5). The result is
process-only: item, machine config, belt, rate, capacity, and the LHS's own
recipe all drop; the RHS recipe is consumed.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **U** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **R** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **M** | `err!` | `err!` | `P` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **P** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **C** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **B** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **T** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **K** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |

### Combination rows (`making`)

| LHS set | RHS set | Result | Condition |
|---|---|---|---|
| `I+M` / `I+M+R` | `R` | `P` | item drops; LHS recipe ignored |
| `I+M` / `I+M+R` | `I+R` | `P` | RHS item drops; RHS recipe consumed; other RHS facets ignored |
| `M` with count `None` | `R` | `P` (unsolved) | displays one machine |
| `M` with count `Some(n)` | `R` | `P` (`Machines(n)`) | `1` counts as an attached count |
| `P` | `R` | `err!` | `making` is not allowed on a process |
| `I` (machine-less item) | `R` | `err!` | no machine facet |
| `M` | `I` / `S` / `C` / `T` / `K` | `err!` | no recipe facet (`m making (2 * :iron-gear-wheel:)` also errors: scaling dropped the recipe) |

### Special conditions (`making`)

- The RHS may carry any non-recipe facets; they are ignored and dropped.
- Validation for the pair: recipe category intersection (including
  `resource_categories` for drills) and productivity gating for the machine and
  its beacon modules; productivity modules are legal only when
  `recipe.allow_productivity` is true. Slot/allowed-effect checks happened
  earlier in construction/`with`.
- A counted machine multiplies per-machine `net`/`power_w` by the count and
  yields `Machines(n)`; an uncounted machine yields an unsolved process whose
  `net`/`power_w` are per one machine.
- The process stores baked effects only, so all configuration must be applied
  with `with` before `making`.
- `making` is part of `MachineExpr`: it attaches after the complete `with` chain
  and takes a single `Primary` on the right. `m making r * 2` =
  `(m making r) * 2`; `2 * m making r` = `2 * (m making r)`;
  `(m1 + m2) making r` needs parentheses.

## `in`

Requires a quantity LHS and a machine with **no** attached count, or an
unsolved process, on the right (§4.6). The LHS
contributes only its item/fluid/craft facet; every other LHS facet drops. The
result is a solved process.

| L \ R | S | U | R | M | P | C | B | T | K |
|---|---|---|---|---|---|---|---|---|---|
| **S** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **U** | `err!` | `err!` | `err!` | `P` (pure item/fluid amount or rate) / `err!` (time, energy, craft without an explicit recipe, multi-unit) | `P` (unsolved) / `err!` (solved) | `err!` | `err!` | `err!` | `err!` |
| **R** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **M** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **P** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **C** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **B** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **T** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |
| **K** | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` | `err!` |

`I+B+T`, `I+C`, `I+K`, `I+R` all behave like `U` on the left (the extra facets
drop); a bare `B`, `T`, `K`, `C`, `R`, `M`, or `P` LHS errors because it is not
a quantity form.

### Combination rows (`in`)

| LHS set / form | RHS | Result | Condition |
|---|---|---|---|
| `I` pure item/fluid **amount** | `M` (count `None`) | `P` (`MachineSeconds(m)`) | `m = coef / abs(k(unit))`; `energy_j = power_w·m` |
| `I` pure item/fluid **rate** (`s⁻¹`) | `M` (count `None`) | `P` (`Machines(m)`) | `net`/`power_w` scaled by `m`; no `energy_j` |
| `I` `craft` amount/rate | `M` | `err!` | craft requires an explicit recipe; a machine cannot supply one — use `in (m making r)` |
| `I` `craft` amount/rate | `P` unsolved | `P` | the process supplies the explicit recipe; `magnitude = coef / k(craft)` |
| `I+R` | `M` | `P` | LHS recipe is ignored; same-id recipe is only a tiebreaker |
| `I+C` / `I+T` / `I+K` / `I+B+T` | `M` | `P` | extra LHS facets drop before resolution |
| `I` pure amount/rate | `P` unsolved | `P` | the process supplies the recipe |
| `I` pure amount/rate | `P` solved | `err!` | solved process is not accepted |
| `I` pure amount/rate | `M` with `Some(n)` (even `1`) | `err!` | a count is attached |
| `I` time value (`second`) | `M` | `err!` | not an amount/rate/craft form |
| `S` scalar | `M` | `err!` | dimensionless scalar is not a pure item/fluid amount |
| `P` | `M` | `err!` | a process is not a machine (`x in (y in m)` errors) |
| anything | `I` / `R` / `C` / `B` / `T` / `K` / `S` | `err!` | no machine/process facet |

### Special conditions (`in`)

- **Purity is about units, not facets:** extra LHS facets are ignored and
  dropped (`:speed-module-3: in ma` is valid). A standalone rate/capacity facet
  with no item facet is not a quantity and errors.
- **Validity depends on attachment, not on magnitude:** a count attached by
  scaling, including `1` (`1 :foundry:`, `0.5 * 2 * :foundry:`) or by `making`
  on a count, is rejected.
- **Recipe selection order:** (1) an unsolved process supplies its baked
  coefficients, with no recipe re-resolved; (2) machine
  auto-resolution — a craftable recipe whose products include the selected unit
  with positive expected amount, filtered by category and productivity gating;
  (3) the same-id recipe only as a tiebreaker among those candidates; (4) zero
  or multiple candidates is an error.
- `k(unit) == 0`, a recipe that does not involve the selected unit, and `craft`
  without an explicit recipe are errors. After resolution, `in` performs the
  same category/productivity validation as `making`.
- Rate → `Machines(m)` (multiplies per-machine `net` and `power_w`); amount →
  `MachineSeconds(m)` (multiplies per-machine `net` by `m`, sets
  `energy_j = power_w·m`, leaves `power_w` absent). The sign of the quantity is
  kept, so a negative quantity yields a negative extent.
- `in` takes a process expression on the right: `with`/`making` are included,
  arithmetic is not — `q in m * 2` parses as `(q in m) * 2`.
- `in` does not use the LHS recipe facet.

## `:stack-size:` prefix (unary)

Requires a pure item quantity (§4.7). Extra facets are dropped; the result is
item-only with the coefficient multiplied by the item's `stack_size`.

| Operand facet set | Result |
|---|---|
| `I` pure item (one item unit at exponent 1) | `I` — coefficient × `stack_size` |
| `I+R` | `I` — recipe drops |
| `I+M`, `I+M+R` | `I` — machine and recipe drop (needs the item's `stack_size`) |
| `I+C` | `I` — config drops |
| `I+T` (inserter), `I+K` (container), `I+B+T` (belt) | `I` — other facets drop; belt is not stacked |
| `I` fluid | `err!` — every fluid is an error |
| `I` multi-unit, `second`, `joule`, `craft`, or empty | `err!` — not a pure item quantity |
| `R`, `M`, `P`, `C`, `T`, `K`, `B+T` alone | `err!` — no item facet |
| item token with no `stack_size` | `err!` |

### Special conditions (`:stack-size:`)

- The result is an item monomial; the belt facet, rate, and capacity never
  survive. `:stack-size: :transport-belt:` is 100 transport-belts as an item.
- The prefix takes exactly one `Primary` and stops at unary `-`, `^`, `making`,
  `with`, `in`, and arithmetic. It binds tighter than juxtaposition, `^`, and
  `*`: `:stack-size: 3 :iron-plate:/s` = `(:stack-size: 3 :iron-plate:)/s`,
  `:stack-size: b1 ^ 2` = `(:stack-size: b1) ^ 2`. `:stack-size: -b1` is an
  error; write `:stack-size: (-b1)`.
- `:stack-size: (b1 :iron-plate:)` errors: the belt application yields a rate,
  not a pure item quantity.
- Stacking level is irrelevant to this prefix; it is a pure item operation.

## `:stacked-belt:` prefix (unary)

Requires a belt facet and its rate (§4.8). Returns an isolated rate at
`Limit(level.clamp(1,4))`; the amount is the belt's existing rate amount, which
already includes its count. Item, belt, recipe, and every other facet drop.

| Operand facet set | Result |
|---|---|
| `I+B+T`, `I+B+T+R` | `T` — cap set to the stacking level; item/belt/recipe drop |
| `B+T` | `T` — cap set to the stacking level |
| anything without a belt or rate | `err!` |

### Special conditions (`:stacked-belt:`)

- The result is isolated: `.rate` reads it, and applying it to a pure item via
  juxtaposition yields an item rate.
- A belt token's rate is unstacked by default (`Limit(1)`); the prefix produces
  the stacked form without retaining the belt facet.
- It takes one `Primary` and binds like `:stack-size:`; unary minus needs
  parentheses.

## Unary `-`

`-` negates quantitative facets; a separate config facet always drops, and the
recipe facet drops (§3.3, §4.1).

| Operand set | Result | Notes |
|---|---|---|
| `S` | `I` | negated scalar |
| `U` | `I` | negated monomial (all exponents kept) |
| `I+R` | `I` | recipe drops |
| `I+M` (count `None`) | `I+M` | item negated; count becomes `Some(-1)` |
| `I+M` (count `Some(n)`) | `I+M` | item negated; count `Some(-n)` |
| `I+M+R` | `I+M` | as above, recipe drops |
| `I+C` | `I` | config drops |
| `I+B+T`, `I+B+T+R` | `I+B+T` | item coefficient, belt count, rate amounts negated; recipe drops |
| `I+T` | `I+T` | item and rate amounts negated |
| `I+K` | `I+K` | item and capacity amounts negated |
| `M` (machine-only) | `M` | count negated (`None` → `Some(-1)`); per-machine modules/beacons fixed |
| `P` solved | `P` | extent, `net`, `power_w`/`energy_j` negated |
| `P` unsolved | `P` (`Machines(-1)`) | solved, matching `(-m) making r` |
| `C` | `err!` | config facet drops, nothing remains |
| `B+T` | `B+T` | belt count and rate amounts negated; cap retained |
| `T` | `T` | every rate amount negated; zero stays a present zero rate |
| `K` | `K` | every capacity amount negated |
| `R` | `err` | recipe drops, nothing remains |

### Special conditions (unary `-`)

- `^` binds tighter than unary `-`: `-2 ^ 2` = `-(2^2)`.
- An uncounted machine becomes `Some(-1)`, and every machine count produced by
  scaling is *attached*, so `-(:foundry:)` cannot be the right operand of `in`.
- Machine per-machine configuration never negates; a standalone config value has
  no negative form and errors.
- Process negation is algebraic and legal at zero and negative extents; an
  unsolved process is solved with `Machines(-1)`, not left unsolved.
