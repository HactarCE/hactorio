# Hactorio expression language

This document describes the expression language as implemented by
`hactorio-core` today. The active implementation is the facet parser and
evaluator (`facet_parser.rs`, `facet_eval.rs`, `facets.rs`, `facet_token.rs`,
`facet_machine.rs`, and `facet_format.rs`). The experimental `new_eval` module
is not the evaluator used by the application.

## Program model

A document is a sequence of lines. Each line is one statement; there is no
statement separator and no expression may continue onto the next line.

- A line whose first non-space character is `#` is a comment.
- A line containing only whitespace is blank.
- Otherwise the line is an expression or a definition.
- `#` starts a comment anywhere outside a `:prototype:` token, and the comment
  runs to the end of the line.
- Spaces and tabs are insignificant between tokens. Newlines are significant.

Evaluation is sequential and stateful:

1. Every line is parsed and then evaluated in order.
2. Definitions add or replace bindings in a variable environment.
3. Later lines see the bindings made by earlier lines.
4. A parse error produces no binding at all.
5. An evaluation error binds the failed name to an error. References to that
   name fail with the stored error until the name is successfully redefined.
   A failed property assignment therefore poisons its variable rather than
   leaving the old value in place.
6. Blank and comment lines produce no result.
7. Each output records the zero-based line number, a hash of the source line,
   the statement span, and either a value, an error, or nothing.

There is no block syntax, function definition, conditional, loop, boolean, or
string type.

## Lexical forms

### Numbers

Numbers are decimal `f64` literals:

- Digits with an optional fractional part: `12`, `12.5`, `.5`.
- A trailing dot is an error: `3.` is rejected.
- No exponent notation (`1e3`) and no sign is part of the literal; `-3` is
  unary minus applied to `3`.
- An optional scale suffix may follow immediately: `k` = 1e3, `M` = 1e6,
  `G` = 1e9, `T` = 1e12. These are case-sensitive. `1k` is 1000; `1K` is not a
  number suffix.
- A number may be followed by a unit keyword, with or without whitespace:
  `3s`, `3 s`, `3kJ`, `3 kJ`.
- A variable or arbitrary word may not immediately follow the digits without
  whitespace. `3x` is rejected; `3 x` is a number followed by a variable.
  A recognized unit keyword is the exception: `3J`, `3s`, and `3craft` are
  valid.

### Unit keywords

| Keywords | Meaning |
|---|---|
| `second`, `s` | time |
| `minute`, `m` | time (60 seconds) |
| `hour`, `h` | time (3600 seconds) |
| `joule`, `J` | energy |
| `kJ`, `MJ`, `GJ`, `TJ` | energy with a decimal multiplier |
| `watt`, `W` | power (joule per second) |
| `kW`, `MW`, `GW`, `TW` | power with a decimal multiplier |
| `craft` | one crafting operation |

A number followed by a unit keyword forms the corresponding literal:
`3 s`, `3 J`, `3 kW`, `3 craft`. A bare unit keyword is a literal of one unit:
`J` is one joule, `s` is one second, `craft` is one craft.

`M` is a numeric multiplier and `m` is minutes; matching is case-sensitive.
Time literals are stored in seconds. `3 minute` and `180 s` denote the same
value.

### Prototype and signal tokens

`:name:` is a prototype token. The name is looked up in the registry; a name
that is not present is an error, as is an empty or unterminated token.
Prototype tokens may also be written as their assigned private-use glyph.
Known signal tokens are recognized before ordinary prototypes:

| Signal | Allowed use |
|---|---|
| `:stack-size:` | prefix operator, or a property after `.` |
| `:stacked-belt:` | prefix operator |
| `:time:` | property after `.` only |
| `:speed:` | property after `.` only |
| `:electric:` | property after `.` only |
| `:module-slots:` | property after `.` only |

A signal glyph is lexed through the same signal table. A prototype glyph is
resolved through the registry glyph assignment.

### Identifiers and variables

An identifier starts with an ASCII letter or `_` and continues with ASCII
letters, digits, or `_`. Hyphens are not allowed in identifiers.

Keywords are reserved and cannot be variables: `in`, `with`, `making`, `joule`,
`J`, `kJ`, `MJ`, `GJ`, `TJ`, `watt`, `W`, `kW`, `MW`, `GW`, `TW`, `craft`,
`second`, `s`, `minute`, `m`, `hour`, `h`.

A valid identifier that names an existing prototype id cannot be used as an
assignment target.

## Expressions and precedence

From lowest binding power to highest:

| Level | Form | Associativity |
|---|---|---|
| 1 | `quantity in machine` | special; see below |
| 2 | `+`, `-` | left |
| 3 | `*`, `/` | left |
| 4 | unary `-` | prefix |
| 5 | `^` | right |
| 6 | juxtaposition (two expressions with no operator) | left |
| 7 | `:stack-size:` and `:stacked-belt:` prefixes | prefix, one primary operand |
| 8 | postfix `.selector` | left |

Parentheses group an expression.

Notable consequences:

- `-2^2` is `-(2^2)`.
- `2^3^2` is `2^(3^2)`.
- `2 * 3 s` is `2 * (3 s)`.
- `2 :iron-plate: 3 s` is the juxtaposition of `2 :iron-plate:` and `3 s`.
- `2 * q in machine` is `(2 * q) in machine`, because `in` binds least.
- `q in machine * 2` is `(q in machine) * 2`.

### Juxtaposition

Placing two expressions next to each other is implicit juxtaposition. It is
tighter than `*` and `^`, and is left-associative. Juxtaposition is also how
prefix operators attach:

```text
:stack-size: :iron-plate:
2 :stacked-belt: :transport-belt:
:transport-belt: :iron-plate:
```

A number immediately followed by a prototype token is not juxtaposition; it is
a quantity literal (`3 :iron-plate:`). It still has the same value as applying
the scalar `3` to the token in most cases, except that quantity construction
and other derived operations drop the token's recipe facet.

### Machine expressions: `with`, `making`, and `in`

`with` and `making` are parsed as part of a machine expression. They bind more
tightly than the ordinary infix operators and are not ordinary binary operators
in the precedence table.

```text
machine with config
machine making recipe
quantity in machine
```

- `with` attaches a config to a machine. Its right-hand side is an expression
  parsed with machine operators disabled, stopping at `with`, `making`, `in`, or
  `)`. This means config arithmetic such as
  `:assembling-machine-3: with 2 :speed-module-3: + :beacon:` is possible, but a
  nested unparenthesized `making` is not.
- `making` takes one primary expression as its recipe.
- Multiple `with` clauses may be chained; `making` terminates the machine
  expression.
- `in` takes a quantity on the left and a machine expression on the right.
  The right side includes any `with`/`making`. A second immediate `in` after
  that machine expression is rejected as non-associative.
- Only the right-hand machine expression of `in` gets this special treatment.
  Ordinary arithmetic after it continues at the normal precedence.

Examples of the grouping:

```text
2 * machine making recipe       # 2 * (machine making recipe)
machine making recipe * 2       # (machine making recipe) * 2
quantity in machine * 2         # (quantity in machine) * 2
quantity in (machine * 2)       # parenthesized machine expression
```

## Values

Evaluation produces a `FacetValue`, not a single number. A value can carry
several independent components:

| Facet | Payload |
|---|---|
| item | a monomial: coefficient plus integer powers of base units |
| recipe | a recipe id |
| machine | a machine id, a config, and an optional count |
| process | a production process |
| config | modules and beacon groups |
| belt | a belt tile count and stack cap |
| rate | item rates by stack cap, or a fluid rate |
| capacity | item capacities by stack cap, or a fluid capacity |

Base units are items, fluids, seconds, joules, and crafts. A monomial stores one
integer exponent per base unit; zero exponents are removed. Values within
`1e-9` of zero are treated as zero by canonicalization and display code. A value
with no facets at all is invalid.

Recipe facets are fragile: they survive construction from a bare recipe token,
but ordinary scaling, juxtaposition with a scalar, arithmetic, unary operations,
and property reads generally drop them. In practice `making` is fed by a bare
recipe token.

A process value is exclusive: it does not coexist with item, recipe, machine,
config, belt, rate, or capacity facets. A machine value and a separate config
value do not coexist; the config is attached with `with`. A belt facet requires
a rate facet.

Item rates and capacities are maps keyed by a stack cap. `Cap::Full` means the
item's own stack size; `Cap::Limit(n)` means `min(n, item stack size)`. Belt
tokens use a cap of 1, inserter tokens use their effective capacity as the cap,
and container tokens use `Full`. The amount stored for an item rate is
multiplied by the effective stack size when the rate is applied to an item.

### Prototype token values

A prototype token starts with the facets present on its registry entry:

- An item entry contributes one item monomial.
- A fluid entry contributes one fluid monomial.
- A recipe entry contributes a recipe facet.
- A machine entry contributes a machine facet with no count and an empty config.
- A module entry contributes an item facet and a config containing one module.
- A beacon entry contributes an item facet and a config containing one beacon
  group with no modules.
- A belt item contributes a belt facet (`count = 1`, cap 1) and an item rate of
  `belt speed * 60 * 8` items per second at cap 1.
- An inserter contributes an item rate based on its cycle time and effective
  capacity. Capacity comes from the prototype and inserter-capacity research;
  the research bonus is clamped to the registry's maximum level. `bulk` and
  `stack_size_bonus` choose the bulk bonus; otherwise the non-bulk bonus is
  used.
- A container contributes item capacity (`Full`, number of slots) or fluid
  capacity when it has a positive fluid capacity.
- A pump contributes a fluid rate of `pumping_speed * 60`.
- An entry with no contributing facet is an error.

A single entry can therefore produce several facets at once. For example the
belt token has an item, belt, and rate facet, and an assembling machine token has
an item and machine facet.

A quantity literal such as `3 :iron-plate:` scales the token's value by the
number. Scaling a config only keeps the config when the factor is a
non-negative integer; module and beacon counts are rounded.

## Arithmetic semantics

Facet operations are component-wise where a rule exists. Components that cannot
be combined are dropped, and the operation fails if no component survives.
Some structural conflicts fail immediately instead of dropping a component, for
example combining processes of different extent kinds or merging configs with
multiple beacon types. This differs from a conventional calculator: an
incompatible item component does not necessarily make the whole operation fail if
another facet can survive.

The parser and evaluator report the dropped facets in errors such as
`` `+` dropped every facet (left: [item], right: [item]) ``.

### Addition and subtraction

- Item facets combine only when their unit maps are identical. Otherwise the
  item facet is dropped.
- Machine facets combine only when the machine id and config are identical.
  `None` counts as 1. The result has an explicit count.
- Process facets combine only when both operands are processes of the same
  solved extent kind (`Machines` or `MachineSeconds`). Counts, net rows, and
  electric load add or subtract. An `Unsolved` process cannot be combined this
  way.
- Config facets merge on addition. Subtraction drops the config facet.
- Belt facets combine only when their caps match.
- Rate facets combine when both sides use the same medium (both item maps or
  both fluids).
- Capacity facets combine by the same medium rule.
- Recipe facets are always dropped.
- Add/subtraction of a process with another kind of value is not a process
  combination and drops the incompatible components.

Approximate-zero rate rows and capacity terms are removed after combination.

### Multiplication

Multiplication is tried in this order:

1. If either operand is a pure dimensionless scalar (an item monomial with no
   units and no other facets), scale the other operand by it.
2. A process multiplied by a pure time value converts to a machine-seconds
   process. The time must be exactly `Second^1` with no other facets.
   Multiplying an existing machine-seconds process by time is an error.
3. An isolated rate multiplied by a pure time value converts to a capacity:
   item rates multiply each cap term by the time, fluid rates multiply the
   fluid amount.
4. Two item facets multiply their monomials: coefficients multiply and unit
   exponents add. Zero resulting exponents disappear.
5. Otherwise multiplication fails.

Scaling affects every facet the value has:

- Item coefficient multiplies.
- Machine count multiplies; an absent count becomes `Some(1 * factor)`.
- Process counts, net rows, and electric load scale. An `Unsolved` process
  becomes a `Machines` process when scaled.
- Config survives only for non-negative integer factors; module and beacon
  counts are rounded.
- Belt count, item/fluid rate terms, and capacity terms scale.
- Recipe is dropped.

### Division

1. If the divisor is a pure dimensionless scalar, the left operand is scaled by
   its reciprocal. This explicitly drops the config facet. Recipe is dropped by
   scaling as well.
2. A process divided by a pure time value converts from `MachineSeconds` to
   `Machines`. A process of another kind divided by time is an error. Dividing
   by a process is unsupported.
3. An isolated capacity divided by a pure time value converts to a rate.
4. An isolated rate divided by a pure time value is unsupported.
5. Isolated rates/capacities divide as follows:
   - rate / rate and capacity / capacity produce a dimensionless scalar;
   - rate / capacity produces `Second^-1`;
   - capacity / rate produces `Second`.
   Item maps must each have at most one term. Different caps are incompatible.
6. Two item facets divide their monomials: coefficients divide and unit
   exponents subtract.
7. Otherwise division fails.

### Exponentiation

`^` requires both the base and exponent to have an item facet.

- The exponent must be dimensionless.
- A dimensionless base may use any real exponent.
- A base with units requires an integer exponent (within the same approximate
  integer test used elsewhere).
- The result is the item facet only. Other facets of the base are not carried
  into the result.
- A dimensional exponent and an overflow of the integer exponent are errors.
  A value with units raised to a fractional power is an error.

### Unary minus and `.abs`

Unary minus negates the item coefficient, machine count, belt count, rate and
capacity terms, and process load/counts. A process is scaled by `-1` and
canonicalized. Unary minus drops config and recipe facets. `.abs` takes absolute
values of the numeric facets and preserves the config facet; recipes are still
dropped. `.abs` refuses to operate on a process.

## Juxtaposition rules

Juxtaposition has these rules, tried in order:

1. **Rate applied to an item/fluid quantity.** If the left operand has a rate
   facet and the right operand is a pure item or fluid amount with one unit at
   exponent 1, the result is an item/fluid amount:
   - item rates sum `amount * effective_stack(cap, stack_size)` across caps and
     multiply by the item count, giving item per second; the item's stack size
     is required;
   - fluid rates multiply the fluid amount, giving fluid per second.
2. **Capacity applied to an item/fluid quantity.** The same rule produces a
   non-rate amount rather than a rate.
3. **Config juxtaposition.** Both operands must have config facets:
   - a single beacon group on the left plus modules on the right attaches the
     modules to that beacon group and keeps the left item facet;
   - two module-only configs merge their module counts and produce a config with
     no item facet;
   - other config shapes are errors.
4. **Scalar scaling.** If either operand is a pure dimensionless scalar, the
   other is scaled by it.
5. Otherwise juxtaposition fails. The error suggests explicit `*` when the two
   values are unit-bearing.

Juxtaposition can therefore be read as “apply this rate/capacity to this
amount”, or “attach these modules to this beacon”, or “multiply by a scalar”,
depending on the operand shapes.

## Machine configuration

Modules and beacons are represented as config facets. A config may contain a
list of module counts and a list of beacon groups; each beacon group has a
beacon count and its own module counts. Counts are canonicalized: zero rows are
removed, modules are sorted by id, and beacon groups are sorted by beacon id and
module contents.

### Config juxtaposition

- `beacon_config module_config` attaches modules to the single beacon group and
  retains the left operand's item facet.
- `module_config module_config` merges the module counts; the result is a
  config-only value.
- A config with multiple beacon groups cannot receive modules this way.
- A beacon config on the right of a module-only config is not a valid
  juxtaposition.

### `with`

`machine with config`:

- The left operand must have a machine facet.
- The right operand must have a config facet.
- The config is merged into the machine's config.
- The merged config is validated against the machine.
- The result contains only the machine facet.

Validation checks module-slot counts, module categories against the machine's
allowed effects, beacon slot counts and effects, multiple beacon types, recipe
category compatibility, and whether productivity is allowed for the recipe.
`with` validates without a recipe; `making` and `in` validate again with the
resolved recipe.

### Machine modifiers

For a machine and config, the implemented modifier calculation is:

- Module effects are summed, filtered by the machine's allowed effects.
- Beacon effects are summed per beacon, multiplied by the beacon count and the
  beacon's `distribution_effectivity`, then multiplied by the profile entry for
  the total beacon count. The profile index is `beacon_count - 1`; a count
  beyond the profile length is an error. Only one beacon type is allowed.
- Beacon-transmitted effects are still filtered by the machine's allowed
  effects.
- Speed is `machine speed * (1 + speed effect)`.
- Productivity is the machine's base productivity plus module productivity plus
  applicable recipe research productivity, capped at `+300%` (a bonus of 3.0).
  Recipe research is only included when a recipe is being used. A recipe that
  disallows productivity contributes no module productivity.
- The consumption multiplier is `1 + consumption effects`, clamped to at least
  `0.2`.
- Active power is `energy_usage * clamped consumption multiplier`.
- Pollution per minute is base pollution times the clamped consumption
  multiplier times `1 + pollution effects`.

## `making`

`machine making recipe`:

- The left operand must have a machine facet.
- The right operand must have a recipe facet.
- The machine and recipe are validated.
- The process net coefficients are computed per second for one craft rate:
  - `crafts_per_second = machine speed / recipe craft time`;
  - each product contributes
    `crafts_per_second * (amount + productivity * (amount - ignored_by_productivity))`;
  - each ingredient subtracts `crafts_per_second * amount`;
  - the craft unit receives `crafts_per_second`;
  - approximately zero rows are removed.
- If the machine has no attached count, the result is an `Unsolved` process with
  one machine spec, net per second, and electric load in watts.
- If the machine has a count, the result is a `Machines` process: the net and
  electric load are multiplied by the count.
- The result contains only the process facet.

## `in`

`quantity in machine` sizes a production process to a requested amount or rate.

The left quantity must be a pure amount with exactly one of:

- one item unit at exponent 1, optionally with `Second^-1`;
- one fluid unit at exponent 1, optionally with `Second^-1`;
- one craft unit at exponent 1, optionally with `Second^-1`.

The optional `Second^-1` determines whether the result is a rate
(`Machines`) or an amount (`MachineSeconds`).

The right operand may be:

- an `Unsolved` process. Its net and electric load are used directly and its
  first machine spec is retained; the process invariant normally gives it one
  spec. A solved process is rejected.
- a machine facet without an attached count. The recipe is resolved from the
  requested non-craft unit:
  - only recipes whose categories intersect the machine's categories and which
    produce the requested unit are candidates;
  - recipes that disallow productivity are excluded when the config contains
    productivity modules;
  - one candidate is used directly; if several candidates exist, a recipe whose
    id equals the item id is preferred; otherwise the error asks for `making`;
  - no candidate is an error.
  - `craft` quantities require an explicit recipe, so they cannot be used with
    this machine-only form.

The denominator is the process net coefficient for `craft` or for the requested
item/fluid unit. A zero denominator is an error. For craft quantities the
magnitude is `coefficient / denominator`; for item and fluid quantities it is
`coefficient / abs(denominator)`.

The resulting process scales the process net and electric load by the magnitude.
A rate quantity produces `Machines`; an amount produces `MachineSeconds`.

## Properties

`value.selector` reads one facet or computes a property. A selector is one of:

- a property name: `rate`, `capacity`, `power`, `energy`, `machines`, `crafts`,
  `productivity`, `prod`, `speed`, `pollution`, `abs`;
- a signal: `:time:`, `:speed:`, `:electric:`, `:module-slots:`,
  `:stack-size:`;
- a prototype token: `.:iron-plate:`.

Selectors can be chained syntactically, though most chains fail at evaluation.

| Selector | Source | Result |
|---|---|---|
| `.rate` | rate facet | the rate facet only |
| `.capacity` | capacity facet | the capacity facet |
| `.capacity` | belt facet | item capacity `count * 8`, preserving the belt cap |
| `.:stack-size:` | pure item | scalar stack size of that item |
| `.power` | machine | modifier power times machine count |
| `.power` | process | watts for `Unsolved`/`Machines`; error for `MachineSeconds` |
| `.power` | item/equipment | item or equipment `power_w` times item coefficient |
| `.power` | beacon config | sum of beacon counts times beacon energy usage |
| `.:electric:` | machine/process | like `.power`, except a `MachineSeconds` process yields joules |
| `.:electric:` | item/equipment | power if present, otherwise stored energy; if both power and stored energy exist it is an ambiguity error |
| `.energy` | `MachineSeconds` process | joules |
| `.energy` | item/equipment | stored energy times item coefficient |
| `.machines` | machine | count, with absent count as 1 |
| `.machines` | `Machines`/`Unsolved` process | total machine count |
| `.machines` | `MachineSeconds` process | error |
| `.:time:` | `MachineSeconds` process | machine-seconds as seconds |
| `.crafts` | process | craft net coefficient: craft/s for `Machines`/`Unsolved`, craft for `MachineSeconds` |
| `.speed` | machine | machine speed after module/beacon effects |
| `.productivity` / `.prod` | machine | productivity bonus, not the `1 + bonus` multiplier |
| `.pollution` | machine | pollution per minute after config effects; not multiplied by machine count |
| `.:module-slots:` | machine | base module slot count |
| `.:iron-plate:` etc. | process | net row for that item/fluid; rate units unless the process is machine-seconds |
| `.abs` | non-process value | absolute value of the facets |
| `.abs` | process | error |

A missing property or a property applied to the wrong value type is an error.
A missing process net row reads as zero.

`:stack-size:` as a prefix is different from `.:stack-size:`:

- The prefix requires an item facet whose item is a pure single item and
  multiplies its coefficient by the item's stack size. It returns only the
  resulting item facet; any other facets the operand had are dropped.
- The dotted form returns a scalar stack size and also requires an item facet
  containing a pure item.
- Neither works for fluids.
- `:stack-size:` cannot take an unparenthesized unary minus; parenthesize the
  operand instead.

`:stacked-belt:` is only a prefix, not a property. It requires both a belt facet
and a rate facet. It clamps the configured belt stacking level to 1–4 and
collapses item rate terms into one `Cap::Limit(level)` term containing the sum;
fluid rates pass through unchanged. The result is a rate-only value. It does not
itself apply an item; use juxtaposition afterward, as in
`:stacked-belt: :transport-belt: :iron-plate:`.

## Definitions and mutation

A definition is an expression followed by one of:

```text
=   +=   -=   *=   /=
```

There is no `^=`.

The left-hand side must be either:

- a variable name; or
- a variable name followed by exactly one assignable selector.

Valid property assignment selectors are:

- `.power`
- `.:electric:`
- `.crafts`
- an item or fluid selector such as `.:iron-plate:`.

Other properties, chained selectors such as `p.power.power`, and non-variable
targets such as `1 = 2` are rejected at parse time. A prototype id cannot be
used as a variable name.

Variable definitions:

- `name = expression` replaces the binding.
- `name += expression`, `-=`, `*=`, `/=`, read the current binding and apply the
  corresponding binary operation.
- A compound assignment to an unknown or poisoned name fails.
- The result is stored whether it is a value or an error.
- Defining the name again clears an earlier error.

Property definitions operate on a process:

- The variable must hold a process facet.
- `.power` and `.:electric:` write the process electric field.
  - `.power` is invalid on a `MachineSeconds` process and expects watts.
  - `.:electric:` expects watts for `Unsolved`/`Machines` processes and joules
    for `MachineSeconds` processes.
  - `*=` and `/=` require a dimensionless scalar right-hand side.
  - `+=`, `-=`, and `=` accept the unit required by the target.
- `.crafts` writes the craft net row. It is invalid when the process has more
  than one machine spec. The right-hand side is craft/s (or dimensionless
  per second) for non-machine-seconds processes and craft (or a dimensionless
  amount) for machine-seconds processes.
- An item/fluid selector writes the corresponding net row. The right-hand side
  must be that unit per second, or dimensionless per second, for non-machine-
  seconds processes; or that unit, or a dimensionless amount, for machine-
  seconds processes.
- `*=` and `/=` on net rows require a dimensionless number. They leave an
  absent or zero row absent. Other operators can create a row. Net rows are
  canonicalized afterward, so approximate-zero rows disappear.
- Writing a property returns the whole modified process, which is bound back to
  the variable.

## Result formatting

Result formatting does not affect evaluation but is part of the calculator’s
observable semantics:

- The first present facet is displayed in this order: item, machine, process,
  config, rate, capacity, recipe.
- Dimensionless values use the plain number style. Item quantities use metric
  prefixes when `metric_prefixes` is enabled. Joules and watts always use SI
  prefixes (`J`, `kJ`, `MJ`, `GJ`, `W`, `kW`, `MW`, `GW`, `TW`).
- Time displays use the configured `time_unit` (`second`/`s`, `minute`/`m`,
  `hour`/`h`). Evaluation itself always stores seconds.
- Energy is displayed as joules and power as watts; power is displayed in watts
  regardless of the global time unit.
- A craft rate is displayed as `craft/s`.
- Item rates in belt display mode are converted to belt counts using the
  configured belt tier, stacking mode, item stack size, and stacking level.
- Item rates list `Full` terms first and then limits in descending order;
  capacities use the same ordering.
- A machine displays an optional count, its glyph, and an optional config
  suffix.
- A process displays its machines inline, then its net rows other than craft,
  separated by ` · `. Machine-seconds machines are marked with `*s`.
- A config displays repeated module glyphs and beacon groups; an empty config
  displays as `0`.
- A recipe-only value displays the recipe’s glyph.
- An empty facet value displays as `0`.

Number display trims trailing zeros after the configured maximum number of
decimals; values below the approximate-zero threshold display as `0`.
