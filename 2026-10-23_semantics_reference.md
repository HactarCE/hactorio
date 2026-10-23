# hactorio semantics reference

Concise human reference. Authoritative details live in
`2026-10-22_design_doc_2.md`; implementation notes live in
`2026-10-20_design_doc_1.md`.

## 1. Syntax

| Element | Rule |
|---|---|
| Statement | `name = expr` or `expr`; one per line |
| Variables | Tab-scoped; cannot be redefined |
| Prototype token | `:prototype-id:` or icon |
| Operators | `+`, `-`, `*`, `/`, `^`, `.`, `in`, `making`, `with` |
| Comments | Start with `#` |
| Numbers | Decimal; `k`/`M`/`G` suffixes |
| Signals | `.:stack-size:`, `.:clock:`, `.:speed:`, `.:lightning:` |

## 2. Facets

A value is a set of independent facets. Each facet has its own coefficient,
count, or map.

| Facet | Example | Single-line display |
|---|---|---|
| `item` | `3 :iron-plate:` | `3 :iron-plate:` |
| `recipe` | `:casting-iron:` | `:casting-iron:` |
| `machine` | `:foundry: with 4 :speed-module-3:` | `:foundry: with 4 :speed-module-3:` |
| `process` | `:foundry: making :casting-iron:` | process summary (one machine) |
| `config` | `4 :speed-module-3: + 2 :beacon: 2 :efficiency-module-2:` | `4:speed-module-3:+2:beacon:2:efficiency-module-2:` |
| `belt` | `:stack-size: :transport-belt:` | `15:stack-size:₄/s` |
| `rate` | `:transport-belt:.rate` | `15/s` |
| `capacity` | `:steel-chest:.capacity` | `48:stack-size:` |

Fluid rate/capacity use volume units (`fluid/s`, `fluid`).

## 3. Value behavior

**Facet survival** (binary operations):

- A facet survives iff both operands have it and the operation is defined.
- Exception: a dimensionless scalar scales every scalable facet, even though it
  does not carry those facets.
- Otherwise, one-sided facets are dropped.
- The `recipe` facet is dropped by any operation.
- The `process` facet never coexists with an `item` or `machine` facet.
- If no facet survives, the operation is an error.
- Dropped facets are silent; the details popup shows survivors.

A process is **solved** when it has a concrete extent (`Machines(n)` or
`MachineSeconds(t)`), and **unsolved** when its extent is absent. An unsolved
process displays and computes as one machine.

Machine and process facets each carry an optional payload:

| Payload | Absent | Present |
|---|---|---|
| machine count | `None`: uncounted, assume 1 | `Some(n)`: counted |
| process extent | `None`: unsolved | `Some`: solved |

### Machine and process

`making` turns a machine into a process; `in` solves a process.

| Form | Result |
|---|---|
| `:foundry:` | item + machine + recipe facets; count `None` (uncounted) |
| `3 :foundry:` | machine count `Some(3)` |
| `m with cfg` | machine + config → machine-only; preserves count |
| `m making r`, `m` uncounted | machine + recipe → process; unsolved; display assumes 1 machine |
| `m making r`, `m` counted | machine + recipe → process; solved `Machines(n)` |
| `q in m` | quantity + uncounted machine or unsolved process → solved process |

- `with`/`making` reject processes. Configure before `making`.
- Process `+`/`-`: solved operands, same extent kind. Unsolved processes do not
  combine.
- Scaling a solved process scales its extent/effects.
- Scaling an unsolved process by `s` (including `1`) solves it as `Machines(s)`.
- `Machines * time → MachineSeconds`; `MachineSeconds / time → Machines`.
  Unsolved `* time` assumes one machine.
- Beacon power is not included in process power; add `:beacon:.power` manually.

### Config

- `+` merges modules and beacon groups.
- `*`/`/` by a unitless scalar scales counts.
- Any other multiplication drops the config facet.
- `-` drops the config facet.
- `with` merges config into a machine; negative counts are invalid.

### Belt

- Token: item + belt + rate, unstacked `Limit(1)`.
- Base slots/s: yellow 15, red 30, blue 45, green 60. At `Limit(1)` this equals
  items/s.
- Capacity: 8 slots per tile, hardcoded.
- Belt stacking level is a research/setting; the display toggle affects display
  only.
- `:stack-size:` belt mode: keeps belt + rate, sets cap to the stacking level,
  drops the item and other facets.
- Belt `+`/`-`: requires the same single rate cap; otherwise belt facet drops.
- `.capacity`: `8 slots/tile × belt count`, using the single rate cap.
- `.capacity` exists only with a belt facet or a native capacity facet.

### Rate and capacity

`Cap = Full | Limit(u32)`; `Full` uses the item's own stack size.

| Facet | Item | Fluid |
|---|---|---|
| `rate` | `Cap → stacks/s` | volume/s |
| `capacity` | `Cap → stack slots` | volume |

- Fluid rates and capacities have no caps.
- `+`/`-`: merge terms; same medium (and caps for items).
- Scaling scales amounts.
- Juxtaposition applies a rate/capacity to an item or fluid. For items,
  `amount = Σ amount × min(cap, item stack size)`; fluids use the volume
  directly.
- Isolated conversion: `rate * time → capacity`, `capacity / time → rate`.
- Isolated division, same medium/caps:

| Operation | Result |
|---|---|
| `rate/rate`, `capacity/capacity` | dimensionless item scalar |
| `rate/capacity` | `1/s` |
| `capacity/rate` | `s` |

- Container capacity: chests, cars, and cargo wagons use `inventory_size`
  slots (`Full`); tanks and fluid wagons use fluid volume; pumps use fluid rate.
- Inserter rate: one `Limit(capacity)` term.
  - Capacity: non-bulk `1 + stack bonus`; bulk `2 + bulk bonus`; stack inserter
    `2 + stack_size_bonus + bulk bonus`.
  - Rate: `ticks_half = ceil(0.5 / rotation_speed) - 1`;
    `cycles/s = 60 / (2 * ticks_half)`; `items/s = capacity * cycles/s`.
  - Inserter rates are based on machine-to-machine transfer with 180° swings,
    which might not be the case for modded inserters.

Display: `Full` → `:stack-size:`, `Limit(n)` → `:stack-size:ₙ`,
`Limit(1)` → no icon.

## 4. Operators

| Operator | Type signature | Notes |
|---|---|---|
| `value . property` | `value → facet` | |
| `a b` | `rate/capacity × item → item quantity`; `config × config → config`; `scalar × value → value` | |
| `a ^ b` | `item × dimensionless → item` | Other facets drop |
| `-a` | `value → value` | Quantitative facets only |
| `m making r` | `machine × recipe → process` | |
| `a * b`, `a / b` | `value × value → value` | Dimensionless scalar scales; isolated rate/capacity division |
| `a + b`, `a - b` | `value × value → value` | Per-facet; config `-` drops |
| `m with cfg` | `machine × config → machine` | |
| `q in m` | `quantity × (uncounted machine | unsolved process) → process` | |
| `:stack-size: v` | `belt rate → belt rate`; `item → item` | |

Juxtaposition tries rate/capacity application, then config merge, then item
multiplication.

## 5. Precedence

Tightest first:

1. `.`
2. juxtaposition
3. `^`
4. unary `-`
5. `making`
6. `*` `/`
7. `+` `-`
8. `with`
9. `in`

Parse notes:

| Form | Groups as |
|---|---|
| `m with sm + bc` | `m with (sm + bc)` |
| `m with sm making r` | `(m with sm) making r` |
| `m making r + x` | `(m making r) + x` |
| `m making r * 2` | `(m making r) * 2` |
| `2 * m making r` | `2 * (m making r)` |
| `q in m making r` | `q in (m making r)` |
| `q in m * 2` | `(q in m) * 2` |
| `:stack-size: 2 :transport-belt:` | stack the 2-belt quantity |
| `:stack-size: b1 * 2` | `(:stack-size: b1) * 2` |

## 6. Properties

| Property | Reads | Result |
|---|---|---|
| `.rate` | Rate facet | Rate-only value |
| `.capacity` | Capacity facet; or belt + rate cap | Capacity-only value |
| `.stack-size` | Item facet with one item | Prototype stack size |
| `.power` | Machine/process count, or item/equipment entity | Watts |
| `.energy` | Solved `MachineSeconds`, or item/equipment stored energy | Joules |
| `.electric` / `.:lightning:` | Machine/process by extent; item by unique power/energy | Power or energy |
| `.machines` | Machine count; process `Machines`; unsolved process | Machine count |
| `.time` | Solved `MachineSeconds` | Seconds |
| `.crafts` | Process | `craft/s` or `craft` |
| `.speed`, `.productivity`, `.pollution`, `.slots` | Machine | Derived scalar |
| `.recipe` | Process with one recipe | Recipe-only value |
| `.:item:` / `.:fluid:` | Process | Signed net row |
| `.abs` | All quantitative facets | Absolute values |

- `.power` reads entity `energy_usage` and equipment
  `energy_consumption`/`power`.
- `.energy` reads `buffer_capacity`/`max_energy`.
- `.power`/`.energy`: strict about `Machines` vs `MachineSeconds`.
- `.electric` errors when an item has both power and stored energy.
- `.recipe` and `.:item:`/`.:fluid:` are process-only.
- `.abs` applies to item/machine/belt/config counts, process amounts, and
  rate/capacity amounts; it drops the recipe facet.

## 7. Recipe resolution and solving (`in`)

`in` is the process solver. It takes a quantity and an uncounted machine or
unsolved process, chooses a recipe, and produces a solved process. A rate yields
a machine count; an amount yields machine time.

RHS: an uncounted machine or an unsolved process. A counted machine or solved
process is rejected.

`making r` on the machine supplies the recipe. Without it, the calculator
chooses the machine-compatible recipe for the selected unit. If none or several
match, specify one with `making`.

For item/fluid units, `in` divides by the absolute net coefficient; for craft,
by the signed craft coefficient. The quotient becomes the machine count (for a
rate) or machine time (for an amount), and the quantity's sign is preserved.

```
:iron-gear-wheel: in :foundry:                                      # casting
:iron-gear-wheel: in (:foundry: making :casting-iron-gear-wheel:)   # explicit
:iron-plate: in (:assembling-machine-3: making :iron-gear-wheel:)    # override
```

## 8. Output

Facet order: item, rate, capacity, machine, process, recipe.

- Single-line: first present of item, machine, process.
- Otherwise rate → capacity → recipe.
- Multiline popup: all facets.
- Process: net rows, machine count/time, craft, power/energy.
- Unsolved process: assume one machine.
- Config-only values display the clause list alone, e.g.
  `4:speed-module-3:+4:beacon:2:efficiency-module-2:`; belt-only values do not
  occur.
- Rate/capacity display uses the stack-size notation; zero displays `0/s`/`0`.

## 9. Examples

```
3 :iron-plate:/s * (5 + 3) s

m = :foundry:
b = :transport-belt:
r = :stack-size: b
(r :gear-wheel: in m).:iron-plate:

3 :foundry: with 2 :speed-module-3: with 4 :beacon: 2 :speed-module-3:
    making :casting-iron-gear-wheel:

:foundry: making :casting-iron:          # unsolved process (one machine)
3 :foundry: making :casting-iron:        # solved Machines(3)
1 * (:foundry: making :casting-iron:)    # solved Machines(1)

:iron-plate: in :foundry:                # allowed: uncounted machine
:iron-plate: in (1 :foundry:)            # error: count attached
:iron-plate: in (:foundry: making :casting-iron:)  # allowed: unsolved
:iron-plate: in (1 * (:foundry: making :casting-iron:))  # error: solved

(2 :beacon:).power
:accumulator:.energy
:stack-size: :iron-ore:                  # one stack of iron ore
100 :transport-belt:.capacity :iron-plate:
(:transport-belt: + :fast-transport-belt:) :iron-plate:
3 :speed-module-3:/s in :electromagnetic-plant:

(:iron-plate: in :foundry:) + (:iron-gear-wheel: in :foundry:)

30:stack-size:/s + 15:stack-size:₄/s + 10/s

2 :iron-plate: + 3 :copper-plate:        # error: no facet survives
```
