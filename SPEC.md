# Hactorio expression language — design specification

Status: agreed in design dialog. Section numbers are stable from this point
on.

```text
0  Target questions
1  Design principles
2  Program model
3  Value model
4  Grammar
5  Operator semantics
6  Facets in depth
7  Display
8  Diagnostics
A  Operator matrix
B  Acceptance examples
C  Decision log
```

## 0. Target questions

The language exists to answer these. Every feature must be justified by at
least one of them, directly or as a component of one.

| ID | Question |
|---|---|
| T1 | I want 3 stacked blue belts of iron plates. How many foundries, with a given module and beacon configuration, do I need? |
| T2 | Given N iron plates, an electric furnace with a given module and beacon configuration, and a given steel-productivity research level: how much steel is produced, and how many seconds does it take? |
| T3 | Given 1k iron plates recycled repeatedly, with a given module and beacon configuration, until rare quality or better: how many recyclers are needed, and how many rare iron plates are produced? |

## 1. Design principles

**P1 — Facets are independent and deliberately redundant.**
A value may carry several facets describing different aspects of the same
thing, and a value with no facets is invalid. Facets may duplicate
information: a transport belt's rate is derivable from its quantity, a bare
`:iron-plate:` carries both a quantity and an identity, a recipe is derivable
from an item. The duplication is kept, not factored out, because operators act
on the facet they address and never reach across facets to reconstruct one.
Locality of reasoning is worth the redundancy.

The exception is the process, which never coexists with any other facet: a
process is a solved production result, not an object with aspects. See §3.4.

**P2 — Reference facets hold identity; magnitude facets hold numbers.**
A reference facet names a prototype and may carry *unspecified* quality. A
magnitude facet carries concrete values, and every quantity term has a
concrete quality. This split is why `:iron-plate:` can have a
common-quality quantity and an unspecified-quality identity at the same time.

**P3 — Operators are lenient and silent.**
An operator combines every pair of facets it knows how to combine, silently
drops the rest, and errors only when nothing survives. Dropping a facet its
partner cannot combine with is the normal case, not an anomaly, so reporting
each drop would bury the result in noise. Total loss is the only error.

**P4 — Grammar is fixed early and completely; semantics grows per facet.**
Precedence and the token set are settled in one pass and do not change when a
facet is added. Adding a facet adds rows to the operator matrix, not syntax.

**P5 — Evaluation is interruptible.**
Every statement boundary, including each loop iteration, checks for
cancellation. A document is never allowed to make the application
unresponsive, however large a loop count it contains. See §2.7.

## 2. Program model

### 2.1 Document structure

A document is a sequence of lines. A newline terminates a statement; no
statement continues onto the next line. A statement is one of:

| Statement | Form |
|---|---|
| blank | whitespace only |
| comment | `#`, to end of line |
| expression | an expression, evaluated and displayed |
| definition | `place ASSIGNOP expression` |
| loop header | `loop expression {` |
| loop end | `}` |

A `{` must be the last non-comment token on its line, and a `}` must be the
only non-comment token on its line. Loop bodies are sequences of statements and
may nest. A `}` with no open loop, a `{` that is never closed, and a loop whose
count does not evaluate are errors.

### 2.2 Definitions

```text
definition := place ASSIGNOP expression
place      := NAME | NAME '.' selector
ASSIGNOP   := '=' | '+=' | '-=' | '*=' | '/='
```

- `=` replaces the binding.
- `+=`, `-=`, `*=`, `/=` read the current binding, apply the corresponding
  binary operator, and store the result. `*=` and `/=` take a dimensionless
  number.
- The selector must be one the specification marks assignable: a process's
  resource row. Other selectors are evaluation errors, which is why they can
  name a value only the evaluation knows.
- Only a process's resource rows are assignable, written as `p.:iron-plate: = …`
  or with a compound operator. An unspecified quality means normal. Assignment
  writes the resource's total in the stored net: `=` replaces what the recipes
  produce minus what they consume for that resource, and the process stores
  that total rather than recomputing it from the rows.
- The value must have exactly the row's unit and item. A machines process takes
  a rate (`1/s`), a machine-seconds process an amount, and neither may carry a
  stack rate or capacity. The quantity may name the row's item, at normal
  quality, or name nothing at all:

  ```text
  p.:iron-plate: += 5/s                          # a machines row
  p.:iron-plate: += 5 :iron-plate:/s
  p.:iron-plate: += 5 :normal: :iron-plate:/s
  p.:iron-plate: += :transport-belt: :iron-plate:  # a belt rate, applied to plates
  p.:iron-plate: += 5 :iron-plate:               # error: an amount, not a rate
  p.:iron-plate: += 5 :rare: :iron-plate:/s      # error: the wrong quality
  p.:iron-plate: += 5                            # error: no rate
  p.:iron-plate: += :transport-belt:             # error: no item
  ```

  A machine-seconds process takes the same forms without `/s`; `5/s` there is
  an error. An `Unscaled` process has no sized rows and cannot be assigned. An
  `Unspecified` process accepts either form, and the presence of `/s` decides
  whether it becomes `Machines` or `MachineSeconds`.

A definition is a **statement**, not an operator. `=` has no precedence and
cannot appear inside an expression, so expressions are never effectful and
`a = b = 1` does not exist.

### 2.3 Environment

One flat, global namespace per document. There are no scopes, no user
functions, and no shadowing. A loop body reads and mutates the same namespace
as the code around it.

### 2.4 Evaluation order

Statements are evaluated in order. A loop body is evaluated in order on each
iteration. Later statements observe every binding made by earlier statements,
including those made inside loop bodies.

### 2.5 Errors

No error stops evaluation. Every statement is evaluated; an error is attached
to the statement that produced it, and evaluation continues with the next
statement. Inside a loop body a failing statement does not prevent the remaining
statements of that iteration from running, but the loop stops after that
iteration (§2.6).

| Situation | Result |
|---|---|
| parse error | the statement produces no result and no binding |
| evaluation error in an expression statement | the error is displayed; state is unchanged |
| evaluation error in a definition | the name is bound to the error; reading the name re-raises the error; a later successful definition of that name clears it |

Poisoning a failed definition is preferred over skipping it because a skipped
definition leaves the previous binding alive, and a stale value is worse than
a propagated error. Continuing past an error is preferred over aborting
because a document is a notebook, not a transaction: independent statements
must not be able to block each other.

### 2.6 Loops

```text
loop N { statement* }
```

- `N` is evaluated once, before the body, and must be a non-negative integer.
  `0` runs the body zero times.
- Each iteration runs the body's statements in order against the global
  environment, so mutations accumulate across iterations.
- A failing statement inside the body affects only that statement, and the
  iteration runs to its end (§2.5). The loop then aborts and reports, on the
  line with `loop`, `loop aborted after N iterations due to error on line L`,
  where `N` counts the iterations that ran and `L` is the first failing line
  inside the body, both one-based and `iteration` singular at `N = 1`.
- The loop produces no value.

Loops exist because iterated production has no closed form *in the language*.
A fixed point could be found by materializing the body as a matrix and raising
it to a high power, but the body is an arbitrary sequence of statements over
arbitrary values, so there is no matrix to raise. For T3 the body is a
process-scaling statement:

```text
R = :recycler: 4 :quality-module-2:
q = :recycling: :iron-plate:
p = 100 :iron-plate: into (R making q)
loop 1000 {
  p += p.out.:iron-plate: < :rare: into (R making q)
}
```

### 2.7 Cancellation

Evaluation runs off the UI thread. A newer document supersedes a running
evaluation: the running evaluation is cancelled and discarded rather than
allowed to finish.

Cancellation is checked at every statement boundary, including each iteration
of a loop body. A loop is therefore interruptible, and a large or mistyped
iteration count cannot freeze the application.

A cancelled evaluation produces no output; partial results are never displayed
or cached.

## 3. Value model

### 3.1 Facets

A `MultiValue` is a set of facets, each optional:

| Facet | Payload | Class |
|---|---|---|
| `quantity` | a `Quantity`: an `Amount` (a plain number, or a coefficient per `Resource`) with integer exponents for `second`, `joule`, and `craft`, and an optional `per` resource | magnitude |
| `stack_rate` | `Stacks`: a `StackLimit → f64` map of item stacks per second, or a fluid volume per second | magnitude |
| `stack_capacity` | `Stacks`: the same, as an amount rather than a per-second rate | magnitude |
| `machine` | a machine prototype, a `Config`, and an optional count | magnitude |
| `config` | a module list and a beacon-group map; a beacon group is a beacon prototype with its quality, a count, and a module list | magnitude |
| `resource` | a `ResourceRef`: item or fluid id plus optional quality | reference |
| `recipe` | a `RecipeRef`: recipe id plus optional quality | reference |

Modules in a `config`, the beacon prototype of each beacon group, and the
prototype in a `machine` carry a quality, since a legendary machine, a
legendary module, and a legendary beacon are all different things. The
`MachineWithConfig` ordering key therefore includes all of them.

Process and quality are not facets: they are separate top-level value kinds,
because they never combine with facets (§3.4).

A transport belt's *rate* is redundant with its quantity, and it is kept as
`stack_rate` because `:stacked-belt:`, `.rate`, and rate application all read
it. There is no `belt` facet: no operator reads one. A belt token carries
`quantity`, `stack_rate`, and `stack_capacity`, and `:stacked-belt:` operates
on a rate at `Limit(1)` without needing to know where it came from.

### 3.2 Two quality domains

Quality appears in two places with different types, and the difference is
load-bearing (P2).

- **Quantity terms** are keyed by `Resource` with a **concrete** `Quality`.
  Writing `:iron-plate:` produces the quantity `1 iron-plate/normal`. There is
  no term with an unspecified quality, so every quantity is unambiguous.
- **Reference facets** carry a **`Option<Quality>`**, where `None` means
  unspecified. Writing `:iron-plate:` produces a resource reference with
  unspecified quality.

This is what makes quality selection expressible:

| Expression | Meaning |
|---|---|
| `v.:iron-plate:` | the iron-plate terms or rows of `v`, of every quality |
| `v.:common::iron-plate:` | only the common-quality iron plates |

Selectors therefore filter on the *reference*, not on the magnitude, which
would have to guess a quality.

Consequences:

- `X in Y` reads the quantity of `X`, never the resource reference. `X`'s
  quantity always has concrete qualities, so recipe resolution is never
  ambiguous on quality.
- `RecipeRef` uses the same `Option<Quality>` shape, where `None` may expand to
  several recipes at `making` time. Unlike resource references, this expansion
  is well defined because a process is keyed by machine and recipe.

### 3.3 Quality operand

A quality value placed next to another value is a **quality operand**. It
rewrites the concrete quality of every item-prototype identity the value
names.

A quality may only be the **left** operand. `:iron-plate: q` is an error, not
a no-op: a quality on the right of a value is almost always a mistake, and
silently ignoring it would hide the mistake. Two qualities do not combine
either: `q1 q2` is an error, since there is no juxtaposition rule for them.

- Every item term of the `quantity` facet is re-keyed to the given quality, in
  the numerator `Amount` and in the `per` resource alike.
- The `resource` reference facet is re-keyed to the given quality. A qualified
  value therefore does not match a selection at a different quality.
- The recipe in a `recipe` facet is re-keyed.
- The prototype in a `machine` facet is re-keyed, and its config is left alone.
  `q (:foundry: 4 :speed-module-3:)` promotes the foundry and keeps its modules,
  which is the question "what if my machines were epic?".
- A `config` facet may only be re-keyed when it names exactly one identity:
  - one module entry whose count is exactly one, with no beacon groups, has that
    module's quality set;
  - one beacon group has that beacon's quality set, whatever the beacon count
    and whichever modules the group already holds. The modules are untouched;
  - anything else is an error, because there is no single thing to promote.
- A counted beacon is therefore promoted whole: `q 2 :beacon:` raises both
  beacons. The qualified-token spelling still promotes only its own prototype,
  because `:rare: :beacon: :beacon:` is one qualified token and one plain one.
- Fluid terms and the non-resource units (`second`, `joule`, `craft`) are
  unaffected; fluids have no quality.
- A value with no item terms is unchanged. This is a no-op, not an error.
- A process value is an error. Process quality is a property of the machine
  and recipe, not a relabeling: with quality modules in play, output quality
  is not "set every item to rare".
- Facets keyed by `StackLimit` rather than by resource carry no quality and
  are unaffected.

```text
:rare: :iron-plate:        # one rare iron plate, rare resource identity
:rare: 2 :iron-plate:/s    # two rare iron plates per second
:rare: 5                   # 5
```

Quality values combine by juxtaposition (§4.2). When the quality is written as
a token directly before a prototype token, the pair is a single qualified
token and no juxtaposition happens (§4.5).

### 3.4 Coexistence

A process never coexists with any other value kind, so the exclusion is
structural:

```rust
enum Value {
    MultiValue(MultiValue),   // every magnitude and reference facet
    Process(Process),         // never combines with the above
    Quality(Quality),         // an operand; see §4.2
}
```

A flat struct with an asserted invariant is the alternative. The nested form
is preferred because the operator matrix is keyed by top-level kind anyway
(§5), so the match is exactly where the matrix already is, and because an
asserted invariant cannot be checked by the compiler.

A machine that has been given a recipe is **not** a fourth kind: it is a
`Process` whose extent is `Unscaled`. `MachineMakingRecipe` is a process map
key, not a value type.

### 3.5 Identity and equality

Equality is structural and exact. `f64` payloads are never map keys, which is
what makes that possible:

- Keys are ids, qualities, stack limits, and tuples of them, all `Ord`.
- Quantities compare exactly: same `per`, same exponents, same amount case,
  equal coefficients. Canonicalization has already removed zero terms.
- Configs compare exactly: sorted module lists and beacon-group maps.
- Machines and processes compare exactly.

The approximate-zero threshold is a *canonicalization and display* rule, not an
equality rule. A value with `|x| < 1e-9` is zero: the threshold applies when a
quantity is constructed, when it is canonicalized, and when it is displayed. A
coefficient inside the threshold is removed when the value is built, or shown as
`0`; equality itself never needs a tolerance.

## 4. Grammar

### 4.1 Juxtaposition chains

Operands placed next to each other with no operator between them form one
**juxtaposition chain**. A chain is parsed as a single n-ary node, not as a
left-nested tree, and is evaluated from the right: `a b c` means `a (b c)`.

There is no separate number-before-token production. `2 :speed-module-2:` is a
chain of a scalar and a token, and the scaling rule turns it into two modules.

```text
:beacon: 2 :speed-module-2:        # beacon with two modules
n = 2
:beacon: n :speed-module-2:        # beacon with two modules
```

The two lines must agree; a scalar is a scalar whether it is written as a
literal or held in a name.

Right-associativity has one visible consequence that is accepted deliberately:

```text
2 :speed-module-2: 3 :productivity-module-2:
# parses as 2 (:speed-module-2: (3 :productivity-module-2:))
# 2 speed modules and 6 productivity modules, not 2 and 3
```

The mistake is normally caught by slot validation, and the fix is either
parentheses or an explicit `+` between the module groups. Left-associativity was
rejected because it makes `:beacon: n :speed-module-3:` disagree with
`:beacon: 2 :speed-module-3:`, which is worse: the same value written two ways
must not mean two things.

A qualified prototype token (§4.5) is one operand, so
`:beacon: 2 :rare: :speed-module-2:` is a three-operand chain whose last
operand is the qualified token.

Unit keywords are operands too: `3 s` is a scalar and a time token, and
`3 kJ` is a scalar and a kilo-joule token.

### 4.2 Quality values

A quality token is an ordinary expression evaluating to a `Quality`.
Qualities are first class so they can be stored and reused:

```text
q = :rare:
q :iron-plate:                 # one rare iron plate
v.:iron-plate: < q             # the iron plates below rare
q.level                        # 2
```

| Property | Result |
|---|---|
| `.level` | the quality's level: `0`, `1`, `2`, `3`, `5` |
| `.multiplier` | `1 + 0.3 × level`: `1`, `1.3`, `1.6`, `1.9`, `2.5` |

`.multiplier` is derived, not read from the registry: the data dump has no
single quality multiplier field, only per-effect ones such as
`beacon_power_usage_multiplier`. The formula reproduces the game's standard
quality scaling.

Apart from these two properties, the filter operators, and juxtaposition, every
operator rejects a quality value, as it rejects most values.

Juxtaposition is the only operator that consumes a quality into another value,
via the quality operand rule of §3.3.

When the quality is a token written directly before a prototype token, the pair
is one qualified token (§4.5) and no juxtaposition happens at all:

```text
:rare: :iron-plate:                        # one atom
:rare: :speed-module-1: :efficiency-module-1:
# a rare speed module followed by a common efficiency module
```

When the quality is a value — read from a variable, say — juxtaposition applies
it to the rest of the chain, because juxtaposition is right-associative:

```text
q = :rare:
q :speed-module-1:              # one rare module
q :speed-module-1: :efficiency-module-1:
# parses as q (:speed-module-1: :efficiency-module-1:)
# error: the config names two module entries (§3.3)
```

The two spellings differ, and the error is the useful outcome: with no single
identity to promote, the language refuses to guess which module was meant. A
config that names one thing promotes cleanly:

```text
q (:beacon: :speed-module-1:)     # a rare beacon with a common module
```

### 4.3 Precedence

From tightest to loosest:

| Level | Form | Associativity |
|---|---|---|
| 1 | prefix: `:stacked-belt:`, `:stack-size:`, `:recycling:` | prefix, one primary |
| 2 | juxtaposition | right |
| 3 | `.` (a property, a signal selector, or a resource operand) | left |
| 4 | quality filters `<` `>` `<=` `>=` `==` `!=` | left |
| 5 | `^` | right |
| 6 | unary `-` | prefix |
| 7 | `/` | left |
| 8 | `*` | left |
| 9 | `-` | left |
| 10 | `+` | left |
| 11 | `making` | left |
| 12 | `in` / `into` / `from` | left |

`=` is not in this table: it is a statement (§2.2), not an operator.

Juxtaposition is the tightest *binary* operator. `.` applies to the whole
juxtaposition chain, so `:common: :iron-plate:` is one selector operand, and
`.` chains to the left: `a.b.c.d` is `(((a.b).c).d)` whether the selectors are
properties, signal selectors, or resource operands. Quality filters sit above
arithmetic (level 4) because they behave like suffixes on a quantity rather
than comparisons of magnitude; chaining them reads naturally, and
`value < :legendary: > :normal` keeps the uncommon, rare, and epic terms. Whitespace around the dot
is insignificant, so `a . b . c` is the same expression as `a.b.c`.

`/` binds tighter than `*` so that a ratio can be written in the numerator
position:

```text
3 :iron-plate: * 1 :gear: / 2 :iron-plate:
# parses as 3 :iron-plate: * (1 :gear: / 2 :iron-plate:)
```

The alternative grouping multiplies two unit-bearing quantities, which is
unrepresentable. `-` binds tighter than `+` for consistency with that pair.

Unary `-` sits between `^` and `/`, which is what makes `-2 ^ 2` equal
`-(2 ^ 2)`: the exponent binds before the negation, while multiplication does
not. Negation may begin any operand, and its right operand extends only over
operators tighter than negation, so `2 ^ -3` is `2 ^ (-3)`. Putting the prefix
operators all at the tightest level would make it `(-2) ^ 2`, silently flipping
a familiar result.

Juxtaposition binds tighter than `making`, so a machine and its config are
assembled before the recipe is attached:

```text
:foundry: 2 :efficiency-module-3: making :iron-gear-wheel:
# parses as (:foundry: (2 :efficiency-module-3:)) making :iron-gear-wheel:
```

There is no `with` operator. Configuration is juxtaposition of a machine with a
config (§5.2), which is why it needs no precedence level of its own and why a
recipe cannot be attached before the machine is configured.

### 4.4 Tokens

**Numbers.** A decimal `f64` literal: digits with an optional fractional part,
so `12`, `12.5`, and `.5` are numbers. A trailing dot is an error (`3.`). No
exponent notation and no sign inside the literal: `-3` is unary minus applied
to `3`. An optional scale suffix may follow the digits immediately: `k`, `M`,
`G`, `T` for 1e3, 1e6, 1e9, 1e12. Case matters, so `1k` is 1000 while `1K` is
an error: a word that is neither a scale suffix nor a unit cannot follow a
number. A word that is both a suffix and the start of a unit is read as the
unit, so `1MJ` is one megajoule; the two readings would give the same value.

A leading dot makes a number: `.5` is a number, so `v.5` is `v 0.5`
(juxtaposition), not a selector.

**Unit keywords.** Each is a token carrying its magnitude:

| Token | Meaning |
|---|---|
| `second`, `s` | one second |
| `minute`, `m` | one minute |
| `hour`, `h` | one hour |
| `joule`, `J` | one joule |
| `kJ`, `MJ`, `GJ`, `TJ` | 1e3, 1e6, 1e9, 1e12 joules |
| `watt`, `W` | one watt |
| `kW`, `MW`, `GW`, `TW` | 1e3, 1e6, 1e9, 1e12 watts |
| `craft` | one craft |

`M` is a numeric multiplier and `m` is minutes; matching is case-sensitive.
These words are reserved and cannot be variable names.

**Prototype tokens.** `:name:` resolves through the registry, or the assigned
private-use glyph. An empty or unterminated token is an error, as is a name the
registry does not have. Signal tokens are recognised before ordinary
prototypes.

**Signal tokens.** `:stack-size:` and `:stacked-belt:` are prefix operators;
`:time:`, `:speed:`, `:electric:`, `:module-slots:`, `:item:`, and `:fluid:`
are only valid after a `.`; `:recycling:` is a prefix operator. A signal glyph
lexes through the same table.

**Facet selectors.** After a `.`, these words are properties rather than
identifiers: `rate`, `capacity`, `machines`, `crafts`, `productivity` (or
`prod`), `pollution`, `power`, `energy`, `abs`, `level`, `multiplier`, `in`,
`out`, and `net`. Everywhere else the first eleven are ordinary identifiers,
and `in` is a keyword.

**Quality tokens.** `:normal:`, `:common:`, `:uncommon:`, `:rare:`, `:epic:`,
`:legendary:`, and `:quality-1:` through `:quality-5:` (§4.6). Recognised
before ordinary prototypes, and may be followed by a prototype to form a
qualified token (§4.5).

**Identifiers.** `[A-Za-z_][A-Za-z0-9_]*`. Hyphens are not allowed. The scale
suffixes `k`, `M`, `G`, and `T` are not reserved: they only occur after a
number or before a unit, so `k` is an ordinary variable name. Reserved
words are the unit keywords above plus `in`, `into`, `from`, `making`, and
`loop`. An identifier that names an existing prototype id is not a valid
definition target.

**Comments and whitespace.** `#` starts a comment that runs to end of line; no
token contains `#`. Spaces and tabs separate tokens and are otherwise
insignificant. Newlines terminate statements.

**Punctuation and operators.** `(`, `)`, `{`, `}`, and the operators
`+ - * / ^ < > <= >= == != .`, plus the definition operators `= += -= *= /=`.

**Adjacency.** A number may be followed with no whitespace by a unit keyword or
a `:token:` (`3s`, `3kJ`, `3:iron-plate:`), and nothing else may: `3x` is an
error, while `3 x` is a number and a variable. This is the only
whitespace-sensitive rule in the language.

### 4.5 Qualified prototype tokens

A quality token directly followed by a prototype token is **one token**, a
qualified prototype. The qualification happens during lexing, so no
precedence rule and no lookahead are involved: there is no juxtaposition
between them.

```text
:beacon: :rare: :speed-module-2:
# parses as :beacon: (qualified speed-module-2 / rare)
# one common beacon with one rare module
```

This is what makes the rule "a quality followed by an item icon applies to the
item icon" hold everywhere, including when the pair is an operand of a
juxtaposition chain or the operand of a `.` selector.

A quality token *not* followed by a prototype token is an ordinary value
(§4.2), which is what allows `q = :rare:` and `v < q`.

Lexing is per line, so qualification cannot cross a newline: a line ending in
a quality token followed by a prototype on the next line has a quality value
and a prototype, not a qualified token.

### 4.6 Quality names and aliases

The registry's quality entries are named `normal`, `uncommon`, `rare`, `epic`,
and `legendary`, with levels `0`, `1`, `2`, `3`, `5`. The following aliases
resolve to the same qualities:

| Spelling | Quality | Level |
|---|---|---|
| `:normal:` `:common:` `:quality-1:` | normal | 0 |
| `:uncommon:` `:quality-2:` | uncommon | 1 |
| `:rare:` `:quality-3:` | rare | 2 |
| `:epic:` `:quality-4:` | epic | 3 |
| `:legendary:` `:quality-5:` | legendary | 5 |

`:common:` exists because `normal` is easy to mistake for "no quality".
`:quality-N:` is the ordinal position, one-based.

Autocomplete must offer the aliases: typing `q1` … `q5` surfaces
`:quality-1:` … `:quality-5:` as the top suggestion.

## 5. Operator semantics

### 5.1 Reference facets

Only a small set of operators read a reference facet; every other operator
drops it. This is a single normative rule and not a per-operator decision.

| Operator | Reference facet it reads |
|---|---|
| `making` | `recipe` on its right operand |
| `:recycling:` | `resource`, and only an item reference |
| `:stack-size:`, `.:stack-size:` | `resource`, and only an item reference |
| quality operand (§3.3) | rewrites `resource` and `recipe` |

`.` and the quality filters do not read a reference facet: they select quantity
terms or process rows, which carry their own concrete quality (§3.2), and they
ignore the reference entirely.

Scaling therefore destroys an item's identity. `3 :iron-plate:` has no
`resource` facet, so `:stack-size: (3 :iron-plate:)` is an error. That is
deliberate: it reads as "the stack of three iron plates", which is not what a
user means. Write `3 :stack-size: :iron-plate:` for three stacks.

### 5.2 Juxtaposition

Juxtaposition's rules are ordered pairs: a facet on the left and a facet on the
right, with the pair deciding the result. The keyword operators (`making`,
`in`, `into`, `from`) and `.` are ordered pairs for the same reason, so
directionality is ordinary in this language rather than something juxtaposition
does to other operators.

A value's facets are combined pairwise (P3), so a facet with no rule for its
partner is silently dropped.

| Left | Right | Result |
|---|---|---|
| scalar | any | the right operand scaled by the scalar |
| any | scalar | the left operand scaled by the scalar |
| `stack_rate` or `stack_capacity` | item or fluid quantity | the applied amount |
| a value whose prototype is a beacon | a module list | the modules fill that beacon's one group, whatever its count |
| a module list | a module list | the merged module list |
| `machine` | config | the machine with that config merged in |
| quality | any | the quality operand of §3.3 |

Everything else drops, and a value with no applicable pair is an error. A
process juxtaposes only with a dimensionless number; two processes, or a
process and a facet-bearing value, are errors.

Applying stacks to an item multiplies each limit's amount by `min(limit,
stack_size)`, sums the limits, and multiplies by the item's coefficient. A
fluid map scales the fluid amount directly. That is the only conversion between
stacks and items, and it is what makes a belt's `1:stack-size:/s` at `Limit(1)`
mean one item per second.

The item facets need no rule of their own. In `:beacon: :efficiency-module-1:`
the left has an item and a config, the right has an item and a config; the
pair `item × item` has no rule and drops, the pair `config × config` attaches,
so the result is config-only. The same applies to any config-shaped value: the
item facets of both operands drop because nothing combines them, which is the
ordinary consequence of P3 rather than a special case.

Building a config from tokens therefore needs no syntax of its own:

```text
:foundry: 2 :efficiency-module-3:
:foundry: (3 :efficiency-module-3: + :speed-module-3:)
:foundry: 2 :beacon: :speed-module-3:
```

Each machine-config juxtaposition validates the merged config against the
machine: module slot counts, allowed effects, beacon slot counts, and beacon
profile length.

Juxtaposition can only build a module list or fill a beacon that has no modules
yet. Every other combination is a **general config addition** and requires
`+`, which adds module counts and beacon-group counts while leaving the module
lists inside each beacon group alone. So `:speed-module-2: + 2 :beacon:` is
valid and `:speed-module-2: 2 :beacon:` is an error.

### 5.3 Quantity arithmetic

A quantity is

```text
amount · second^s · joule^j · craft^c / per
```

where `amount` is a dimensionless number **or** a linear combination of
resources (a bundle), `per` is at most one resource, and the exponents are
signed integers. A number and a bundle are mutually exclusive.

Two limits follow from that shape, and they are the source of most drops
below:

1. A scalar and a bundle cannot be added: `3 + 2:iron-plate:` is
   unrepresentable.
2. A quantity cannot hold two resources in the denominator, nor the product of
   two bundles.

A quantity whose numerator amount is approximately zero carries no units at
all: amount `0`, no `per`, zero exponents. The threshold is `1e-9` on the
absolute value (§3.5), so `0:iron-plate:` and `0/s` are the number `0`.

#### `+` and `-`

- Both operands must agree on `per` and on all three exponents. Otherwise the
  quantity facet drops.
- Both amounts must be the same case: number with number, bundle with bundle.
  A mixed pair drops the facet.
- Bundles add term-wise, so `2:iron-plate: - 1:copper-plate:` is the bundle
  `{iron: 2, copper: −1}`. This is the only way to build a multi-resource
  quantity.
- A zero amount is the additive identity whatever shape it was written in, so
  `:iron-plate: + 0` and `:iron-plate: + 0 :iron-plate:` are one iron plate.
- Zero terms are removed, and a bundle that loses every term becomes the
  number `0`.

#### `*` and `/`

Multiplication adds exponents and division subtracts them.

| Left amount | Right amount | Result |
|---|---|---|
| number | number | the product |
| number | bundle | the bundle scaled by the number |
| bundle | number | the bundle scaled by the number |
| bundle | bundle | drops the quantity facet — a product of bundles is unrepresentable |

- At most one operand may have a `per`. Two `per`s drop the quantity facet,
  whether they name the same resource (`X⁻²`) or different ones (`X⁻¹Y⁻¹`).
- A quantity with one item in the numerator never keeps that item in `per`:
  the two cancel when the value is constructed, so
  `3:iron-plate:/:iron-plate:` is the number `3`.
- **Cancellation.** Multiplying by a single-term bundle whose resource is the
  other operand's `per` cancels both. This is what makes the ratio idiom work:

  ```text
  3:iron-plate: * (1:gear:/2:iron-plate:)   # 1.5 gear
  ```

- Dividing by a number divides the amount.
- Dividing by a single-term bundle moves that resource into `per`.
- Dividing by a multi-term bundle drops the quantity facet: division by a sum is
  unrepresentable.
- Dividing by a quantity that has a `per` moves that resource into the
  numerator, which is representable only when the dividend's amount is a
  number: `6 / (2:iron-plate:)` is `3/:iron-plate:`.
- Division by zero follows `f64`, producing `inf` or `NaN` rather than an
  error.

#### `^`

The base must have a number amount, and the exponent must have a number amount
and no units.

- A dimensionless base uses `powf` and accepts any real exponent.
- A base with unit exponents requires an integer exponent, which multiplies
  every exponent: `(2 s) ^ 2` is `4 s²`.
- A `per` is allowed only with exponent `1`.
- A bundle base is representable only at exponent `1` and `0`; exponent `-1`
  is allowed when the numerator has at most one resource term, since the
  reciprocal moves that term to the denominator. Anything else drops the
  quantity facet.

Every failure above drops the `quantity` facet; the operator errors only when
no facet survives.

#### Unary `-` and `.abs`

Unary `-` and `.abs` work on a quantity's amount, on a stack rate or capacity,
and on a machine's count, coefficient-wise; `per` and the exponents are
unchanged. Unary `-` also negates a process; `.abs` refuses one. A config has no
negative form and drops, as do the reference facets (§5.1).

#### Filters

`QUANTITY CMP QUALITY` for `CMP` in `<`, `>`, `<=`, `>=`, `==`, `!=`. The right
operand must be a **quality**; a number or a quantity there is an error. These
are quality filters, not comparisons of magnitude, and `==` is not an equality
operator: it filters the terms whose quality is the given one.

Autocomplete after a comparison operator offers the qualities alone, since
they are the only legal right operand. A lone `=` is a definition (§2.2), not a
comparison, so it keeps the ordinary completions.

#### Quality filters

- Item terms are kept when their quality satisfies the comparison and dropped
  otherwise.
- Fluid terms are dropped: a fluid has no quality, so none can satisfy a quality
  comparison. `.:fluid:` takes the fluid part before filtering, and `.:item:`
  discards it deliberately.
- A dimensionless amount has no terms to filter, so the result is `0`.
- `per` is filtered the same way; if it does not satisfy the comparison the
  result is `0`.
- Exponents are unchanged, and the filter returns only the `quantity` facet:
  every other facet drops (§5.1).

```text
p.out.:iron-plate: < :rare:     # the rows below rare quality
```

#### `.` selection on a quantity

`QUANTITY . RESOURCE` keeps the terms whose resource matches. A selector with
unspecified quality matches every quality; a qualified selector matches only
that one.

- If no term matches, the result is the number `0`.
- `per` is kept when it matches; the result is `0` when it does not.

`.:item:` and `.:fluid:` keep only the terms of that medium. They are how a
mixed quantity is separated, and they are what a quality filter leaves you
needing, since it drops fluids.

### 5.4 Machine and process operators

#### `making`

`MACHINE making RECIPE`.

- The left operand must have a `machine` facet. Its config is already merged by
  juxtaposition, which binds tighter.
- The right operand must have a `recipe` facet. Its other facets are ignored.
- The result is a `Process` with extent `Unscaled`, or `Machines(n)` when the
  machine carried a count.
- The recipe's quality `None` expands to every quality, giving one row per
  quality — but only for a recipe with an item ingredient. A recipe fed only by
  fluids cannot be made above normal quality, so it has one row at normal and
  naming a higher quality is an error.
- Validation: the recipe's categories must intersect the machine's, and
  productivity modules are permitted only when the recipe allows productivity.
- Every other facet of both operands drops.

#### `into`, `from`, `in`

`QUANTITY into SIZED`, and likewise `from` and `in`.

- The left operand must be a quantity of items or fluids, or carry the
  `craft` exponent.
- The right operand is an `Unscaled` process, or a machine from which the
  recipe is inferred. A process of any other extent is an error.
- `into` sizes by the recipe's input consumption of the selected resource,
  `from` by its output, `in` by its net output. Zero rows never exist — the
  operation that computes them removes them — so a resource the recipe does not
  involve in the relevant row is absent, and sizing by it is an error.
- The process stores its net rows, so a resource the rows both consume and
  produce appears only in the net. `in` and `out` are the parts of the net below
  and above zero, not separate sums.
- A **zero magnitude** is a zero process, not an error: sizing with a magnitude
  of zero yields a process whose extent is `Unspecified` and whose rows are
  empty. Nothing needs to be selected, so no recipe is resolved. This is what
  keeps T3's loop alive — `p.out.:iron-plate: < :rare: into R` becomes `0` in a
  late iteration, and it must add nothing rather than poison `p`.
- A quantity with several resource terms scales one process row per term, and
  each term's concrete quality selects the recipe's quality. Two terms that
  select the same recipe are an error, since the magnitude would be counted
  twice. A machine on the right infers one recipe per term, so a machine that
  makes several things is sized for all of them at once.
- A quantity of zero sizes nothing: the result is a process with no rows and the
  extent `Unspecified`, because whether the caller meant machines or
  machine-seconds cannot be known. An empty process is the identity under `+`,
  so a loop accumulator survives a zero iteration.
- The left operand's unit decides the result extent: a rate (`second^-1`) gives
  `Machines`, an amount gives `MachineSeconds`.
- Recipe selection uses the quantity's concrete resources, never the resource
  reference (§3.2).
- Inference from a machine considers the recipes the machine can make whose
  relevant coefficient for the selected resource is positive, after category
  and productivity filtering. One candidate is used; zero candidates is an
  error; several candidates are an error that asks for `making`.
- Hidden items and hidden recipes are excluded from autocomplete; they stay in
  the registry, so naming them (`:recycling:`) or inferring them from a machine
  still works.
- The result contains exactly the rows the quantity's terms select. Rows of the
  input process that no term selects are dropped rather than left unscaled.
- A solved process is rejected: these operators size a process once.

#### Process arithmetic

- `+` and `-` require both operands to have the same extent kind, and add the
  row scales. An `Unspecified` process is the identity under `+`.
- `*` and `/` by a dimensionless number size the process: a counted machine or
  a scaled process picks one recipe — the only row, else the
  normal-quality row among the quality variants of one recipe, else an error.
  Without that, `2 :foundry: making :transport-belt:` would multiply the five
  quality variants instead of choosing one.
- `Machines(n)` multiplied by a time quantity becomes `MachineSeconds(n·t)`.
- `MachineSeconds(τ)` divided by a time quantity becomes `Machines(τ/t)`.
- `Unscaled` combined with anything is an error.

### 5.5 Config arithmetic

- `+` merges: module counts add per (module, quality), and beacon-group counts
  add per (beacon, beacon quality, module list).
- `-` always drops the config facet: counts are not signed.
- `*` by a positive integer scales the module and beacon counts. Any other
  factor drops the config facet; the operator errors only when no facet
  survives. This is how `2 :beacon: 4 :speed-module-3:` becomes two beacons.
- `/` drops the config facet: a configuration has no division.

## 6. Facets in depth

### 6.1 Stacks and stack limits

`stack_rate` and `stack_capacity` hold `Stacks`: a `StackLimit → f64` map for
items, or a single `f64` for fluids. The two media never mix; mapping item
stacks onto a fluid or the reverse is an error.

A `StackLimit` is either `Full`, meaning the item's own stack size, or
`Limit(n)`, meaning `min(n, stack size)`. `Limit(0)` is invalid.

- A belt token carries a rate and a capacity, both at `Limit(1)`: the rate is
  `belt speed * 60 * 8` items per second and the capacity is 8 stacks.
- `:stacked-belt:` uses the belt-stacking level, one through four. The level is
  a setting that the stacking research drives; it is capped at four.
- A value with no stack facet loses every facet under `:stacked-belt:`, and the
  operator therefore errors.
- `:stacked-belt:` requires every stack facet to be at `Limit(1)` and collapses
  each of them to a single term at the configured stacking level, so a belt
  yields a stacked rate and a stacked capacity together. A fluid rate is an
  error, since it has no stack limit, and so is any other limit: stacking is a
  belt operation. An inserter's rate is incidentally at `Limit(1)` before any
  capacity research, so it is accepted there too; that is semantically
  meaningless and deliberately not special cased.
- An inserter carries a rate of one transfer per cycle, at a cap equal to its
  effective capacity: base 1, or 2 for a bulk inserter, or `2 + stack_size_bonus`
  for a stack inserter, plus the inserter-capacity research bonus, clamped to
  the levels the registry has. Its `hand_size` is cosmetic.
- A container carries a capacity: `slots` stacks at `Full`, or its fluid volume
  when it has one. Chests, cargo wagons, fluid wagons, and storage tanks all
  reach this rule through the same prototype data.
- A pump carries a fluid rate of `pumping_speed * 60`. The offshore pump is its
  own prototype type but reaches the same rule.
- A barrel carries a fluid capacity of 50, the amount the barreling recipe
  fills it with. The filled barrels carry the `barrel` subgroup but the empty
  one is an `intermediate-product`, so the item id decides: `barrel` or a name
  ending in `-barrel`.

Zero terms are removed, but a stack facet stays present: a rate that cancels to
zero is `0:stack-size:/s`, not an absent facet. That is the stack facets' rule
alone; a `quantity` that cancels to zero is the number `0`, with no units
(§5.3).

### 6.2 Config

A config is a module list plus a beacon-group map.

- The module list is sorted by (module id, quality) and holds a count per
  entry. Zero counts are removed.
- A beacon group is keyed by (beacon id, beacon quality, its module list) and
  holds a beacon count.
- A config is never empty: a value with no modules and no beacons has no
  `config` facet. An empty config can therefore not be produced; were one
  displayed, it would read `(empty config)`.

Validation happens when a config is attached: the machine's module slot count
and allowed effects, each beacon's module slot count and its `allowed_effects`,
and — once a config is used for production — the beacon profile length for the
total beacon count. A vanilla beacon allows speed and
efficiency modules only, never productivity or quality, and the data says so.

A config may contain several beacon types, including different qualities of the
same beacon prototype — those are distinct types for every purpose, including
the ordering key. The distribution profile depends only on the total beacon
count, not on the mix.

### 6.3 Machine

A machine is a prototype with a quality, a config, and an optional count. An
absent count means one machine that has not been explicitly counted, which is
what distinguishes `:foundry: making r` from `1:foundry: making r`. The
`MachineWithConfig` ordering key is the prototype, its quality, and the config.

A machine is displayed before its quantity, because a machine token carries
both and the quantity is the redundant one: rendering the quantity would print
`1:foundry:` for `:foundry:`, which parses back with a count attached.

Machine modifiers:

- Module effects are summed and filtered by the machine's allowed effects.
- Beacon effects are summed per beacon, multiplied by the beacon count and the
  beacon's `distribution_effectivity`, then multiplied by the profile entry for
  the total beacon count. The profile index is `beacon_count - 1`.
- A quality module's effect is the chance to upgrade each product by one
  quality level. The first roll upgrades one level with the machine's quality
  chance `Q`; each further level is a constant 10% roll. The base tier takes
  `1 − Q`, a non-top tier `k` takes `0.9 × Q × 0.1^(k−1)`, and the top tier
  takes `Q × 0.1^(n−1)`, where `n` is the number of tiers above the base; the
  game documents the mechanism at
  <https://wiki.factorio.com/Quality#Quality_modules>. At `Q = 10%` the shares
  are `90%` normal, `9%` uncommon, `0.9%` rare, `0.09%` epic, and `0.01%`
  legendary. Each product is split across those qualities, which is why `.out`
  reports one row per quality. A recipe made at a quality starts its upgrades
  from there.
- Quality multiplies a machine's speed, a beacon's transmitted share, and the
  one effect a module exists for: a speed module's speed, a productivity
  module's productivity, an efficiency module's consumption, a quality
  module's quality. A module's downsides — a productivity module's extra
  consumption, a speed module's power draw — do not grow with quality.
- Speed is `machine speed × quality multiplier × (1 + speed effect)`.
- Productivity is the machine's base, plus module productivity, plus research
  productivity when a recipe is in play, capped at `+300%`. A recipe that
  disallows productivity contributes none.
- The consumption multiplier is `1 + consumption effects`, clamped to at least
  `0.2`. Active power is `energy_usage × multiplier`.
- Idle draw is the prototype's `energy_source.drain`. An electric crafting
  machine without an explicit drain draws `energy_usage ÷ 30`, which is the
  game's default; a machine with another energy source draws nothing. Idle draw
  is not affected by modules, beacons, or the consumption multiplier.
- Reported power is active power plus idle draw, as the `idle_power` setting
  chooses (§7.8). `Ignored` adds nothing, `Fractional` adds the idle draw for
  the fractional machine count, and `Rounded up` adds it for the count rounded
  up to whole machines, using the count itself when it is within `1e-9` of an
  integer. A machine-seconds process has no machine count, so its idle part
  always uses the fractional scale.
- Pollution per minute is base pollution × consumption multiplier ×
  `(1 + pollution effects)`.

### 6.4 Process

```rust
struct Process {
    extent: Extent,                                    // Unscaled | Machines | MachineSeconds | Unspecified
    rows: BTreeMap<(MachineWithConfig, Recipe), f64>,  // scale in the extent's unit
    net: BTreeMap<Resource, f64>,                      // recipes minus consumption, plus assignment
}
```

- `Unscaled`: one machine of each row, not yet sized.
- `Machines`: the row scale is a machine count, and the process produces
  continuously.
- `MachineSeconds`: the row scale is machine-seconds of work, so the process
  produces a total amount.
- `Unspecified`: an empty process, from sizing by zero. It is the identity under
  `+`.

The extent is the *kind*; the magnitudes live in the rows, so a process can
hold several machines making different recipes. Rows of one recipe that differ
only in recipe quality are alternatives rather than a sum, so `.machines`
counts them once.

An unspecified recipe quality expands to one row per quality at `making`. That
is why `making :iron-plate:` puts five rows in the process and its totals are
one per quality; naming the quality, as in `making (:normal: :iron-plate:)`,
restricts it to one.

Machine-seconds is an extent magnitude, not a unit. Quantities never carry it;
`.:time:` converts it to seconds, and the wall-clock duration is that over the
chosen machine count, which the language does not pick for you.

Per-row rates still come from the machine, config, and recipe, so they stay
correct when settings change (research levels in particular). The net is
materialised when the process is built and stored in the `net` field, and
assignment edits it in place, so changing a setting re-evaluates the document
rather than leaving a stale net. Productivity applies to products that are not
`ignored_by_productivity`.

Process selectors return a `Quantity` with one term per resource and a
concrete quality, carrying the process's units (`second^-1` for `Machines` and
`Unscaled`, none for `MachineSeconds`):

| Selector | Result |
|---|---|
| `.in` | the part of the net below zero, as positive amounts |
| `.out` | the part of the net above zero |
| `.net` | the net itself, outputs minus inputs |

Selectors on an `Unspecified` process return `0` with no units: `.machines`,
`.crafts`, `.power`, `.net`, `.in`, and `.out`. A property that is meaningless
for the extent (`.:time:` on a rate process, `.machines` on a machine-seconds
one) stays an error.

Selection and filtering compose: `p.out.:iron-plate:` is the iron-plate output
rows of every quality, and `p.out.:iron-plate: < :rare:` is the subset below
rare quality.

### 6.5 Resource and recipe references

`ResourceRef` is an item or fluid id with an optional quality; `RecipeRef` is a
recipe id with an optional quality.

They carry identity only, are read only by the operators in §5.1, and are
dropped by everything else. In a quantity, identity lives in the term keys
instead, with concrete qualities.

### 6.6 Selectors

A `.` takes any expression that names a resource, so `p . q` selects with a
variable and `p . (:normal: :iron-plate:)` with a parenthesized value.

`in` is both an operator and a process property: after a dot it is the
property, and the highlighter treats it as one rather than as a keyword. The
same holds for the other direction: after a dot, a word is never an operator. When the
name is a known property — `rate`, `capacity`, `power`, `energy`, `machines`,
`crafts`, `productivity`, `pollution`, `abs`, `level`, `multiplier`, `in`,
`out`, `net` — it is a property instead, and a signal token such as
`.:stack-size:` is a signal selector.

`Unscaled` and `Machines` processes report rates, because they describe
continuing production. `MachineSeconds` reports totals, because it describes
finished work. A property with no meaning for its operand's extent is an error,
never a silent zero.

| Selector | Applies to | Result |
|---|---|---|
| `.rate` | any value with a `stack_rate` | that facet alone; all other facets drop |
| `.capacity` | any value with a `stack_capacity` | that facet alone |
| `.machines` | `machine` | the count, absent count as `1` |
| `.machines` | process, `Unscaled` or `Machines` | total machine count |
| `.machines` | process, `MachineSeconds` | error |
| `.crafts` | process | the craft rate or total, as a quantity with the `craft` exponent: per second for `Unscaled`/`Machines`, total for `MachineSeconds` |
| `.productivity`, `.prod` | `machine` | the productivity multiplier, `1 + bonus` |
| `.productivity`, `.prod` | process | the shared value; error when rows disagree |
| `.pollution` | `machine` | pollution per minute after config effects, not multiplied by the count |
| `.pollution` | process | per minute for `Unscaled`/`Machines`, total for `MachineSeconds` |
| `.power` | `machine` | machine power × count, plus idle draw per §6.3 |
| `.power` | process, `Unscaled` or `Machines` | total watts |
| `.power` | process, `MachineSeconds` | error |
| `.power` | item | the prototype's `power_w` × coefficient |
| `.energy` | process, `MachineSeconds` | total joules |
| `.energy` | item | stored energy × coefficient |
| `.:electric:` | process | watts for `Unscaled`/`Machines`, joules for `MachineSeconds` |
| `.:electric:` | item | `power_w` when present, else stored energy |
| `.:time:` | process, `MachineSeconds` | machine-seconds as a time quantity |
| `.:speed:` | `machine` | speed after module and beacon effects |
| `.:speed:` | process | the shared speed; error when rows differ |
| `.:module-slots:` | `machine` | the prototype's module slot count |
| `.abs` | any non-process | absolute values; config drops |
| `.:stack-size:` | a value with a `resource` facet naming an item | that item's stack size; not a process, machine, config, or fluid |
| `.:item:` | quantity or process | the item terms only; not a machine or a config |
| `.:fluid:` | quantity or process | the fluid terms only; not a machine or a config |
| `.level` | quality | §4.6 |
| `.multiplier` | quality | §4.6 |
| `.in`, `.out`, `.net` | process | §6.4 |
| `.EXPR` | quantity or process | the matching rows, where the expression names a resource (§5.3, §6.4) |

Notes:

- `.productivity` returns the multiplier `1 + bonus`, which is what a caller
  multiplies output by.
- `.:electric:` exists because it is short and because it works whatever the
  extent: it selects power or energy, whichever the operand has. `.power` and
  `.energy` remain the explicit forms.
- `.:time:` on a `MachineSeconds` process returns the extent in seconds.
  `.machines` does not apply to it, and machine-seconds is not a unit (§6.4).
- `.energy` on a `machine` is an error: a machine has no stored energy. Only an
  accumulator-like item and a `MachineSeconds` process do.
- `idle_power` chooses whether `.power` and `.:electric:` include idle draw
  (§6.3). `.energy` on a machine-seconds process adds idle energy at the
  fractional scale, because that extent has no machine count to round.
- A bare machine has no recipe, so `.power`, `.speed`, and `.productivity` on it
  take module and beacon effects only: no productivity research applies and no
  recipe gate can forbid productivity modules.
- `.pollution` on a `MachineSeconds` process is the total, so it divides the
  per-minute rate by 60.

## 7. Display

Display never affects evaluation, except `belt_stacking`, which the
`:stacked-belt:` prefix reads during evaluation (§7.8). It has three surfaces,
named per the UI vocabulary in `AGENTS.md`:

| Surface | Content |
|---|---|
| single-line result | the lossy one-line projection in the results column |
| multiline result panel | the same value expanded, one facet per line |
| token tip | the plaintext name and details of the prototype under the pointer |

### 7.1 Canonical facet order

The panel lists `machine`, `quantity`, `config`, `stack_rate`, `stack_capacity`,
`resource`, `recipe`. The single-line result takes the first facet present in
the order `machine`, `quantity`, `stack_rate`, `stack_capacity`, `config`,
`resource`, `recipe`.

Machine first because a machine token carries both facets and its quantity is
the redundant one: rendering the quantity would print `1:foundry:` for
`:foundry:`, which parses back with a count attached.

A `process` is exclusive (§3.4), so it replaces the whole result rather than
joining the list.

A `recipe` renders only when it is a value's sole facet, which happens for a
recipe token that is neither an item nor a fluid.

Because a reference facet distinguishes an unspecified quality from a normal
one, a reference facet renders `:common:` explicitly when its quality is
`Some(Common)`, unlike a quantity term, which omits it (§7.5).

| Surface | Content |
|---|---|
| single-line | the first facet present |
| panel | every facet present, in canonical order, one labelled line each |

The single-line result is deliberately lossy: the results column is a fixed
column, and a belt's full rendering
`:transport-belt: · 15:stack-size:/s · 8:stack-size:` does not fit. The panel
shows the facets the single line leaves out.

### 7.2 Numbers

- Coefficients are printed positionally, never in exponent notation.
- Rounding uses `max_decimals` fraction digits when `|x| ≥ 1`, and
  `max_decimals` significant digits when `|x| < 1`. Default 3.
- Trailing zeros are trimmed, and a trailing `.` is never printed.
- A coefficient with `|x| < 1e-9` prints as `0`, so a term of `1e-12` and a
  term of exactly zero are indistinguishable in display. This is the same
  threshold used when a quantity is constructed (§3.5).
- `NaN`, `inf`, and `-inf` print as written.
- `-0` prints as `0`.

| Value | Prints |
|---|---|
| `123456.7891` | `123456.789` |
| `1 / 3` | `0.333` |
| `0.0001234` | `0.000123` |
| `2.0000` | `2` |

### 7.3 Metric prefixes

- Item and fluid amounts, item rates, and stack rates use `k`, `M`, `G`, `T`
  when `metric_prefixes` is enabled, and no prefix otherwise.
- Joules and watts always use SI prefixes (`J`, `kJ`, `MJ`, `GJ`, `TJ`, `W`,
  `kW`, `MW`, `GW`, `TW`), independent of `metric_prefixes`.
- Prefixes never apply to dimensionless numbers, time, or craft.

### 7.4 Time units

`second` renders in the configured time unit, so `180 s` prints as `3 m` when
`time_unit` is minute.

The unit applies wherever time is counted, rates included: a rate prints as an
amount per configured time unit, so `15/s` prints as `900/m` in minute mode.
Rates are not pinned to seconds.

### 7.5 Round-trippable renderings

Quantities, qualities, configs, and machines render in syntax that parses back
to an equal value, so a result can be checked by pasting it back into the
document. Stack limits (§7.6) and superscript exponents are the exception:
subscripts and superscripts are for reading only.

**Quality.** The canonical token, including `:common:`: `:rare:`, `:common:`.

**Quantity.** The coefficient, then its units with no separating space. A
bundle is a sum.

```text
3:iron-plate:
2:iron-plate: + 3:copper-plate:
3:rare::iron-plate:
3:iron-plate:/s
```

- A resource term is the prototype's glyph, preceded by the quality's glyph with
  no separating space. The quality glyph is **omitted when the quality is
  normal**, because a quantity term always has a concrete quality and normal is
  the default.
- A positive exponent other than one follows the unit (`(2 s) ^ 2` renders as
  `4s²`); a negative exponent moves the unit after a `/` (`1 / (2 s)` renders as
  `0.5/s`). Only `second`, `joule`, and `craft` take an exponent; a resource
  term never does.
- `joule` prints as a SI-prefixed energy (`1.5kJ`), `watt` as a SI-prefixed
  power (`2MW`), `craft` as `craft`.

When a quantity has several resource terms, the unit suffix is repeated on
every term: `2:iron-plate:/s + 3:copper-plate:/s`. This parses back to the same
quantity, because addition requires matching exponents, so distributing the
suffix cannot change the value. A denominator resource follows a `/`:
`3:iron-plate:/:copper-plate:`.

**Machine.** The count whenever the machine carries one — `1:foundry:` is a
counted machine and `:foundry:` is an uncounted one, and they are different
values — then the glyph, then the config in parentheses when the config is not
empty. The parentheses are required, not
cosmetic: `+` binds looser than juxtaposition, so a config containing ` + `
would otherwise be an addition to the machine rather than part of the config
attached to it.

```text
:foundry:
1:foundry:
2:foundry:
2:foundry:(:efficiency-module-3:)
:foundry:(:speed-module-3::speed-module-3:)
```

**Config.** No spaces and no parentheses inside a config.

```text
config        := module-list? beacon-group ('+' beacon-group)*
module-list   := (QUALITY? MODULE)*
beacon-group  := COUNT? QUALITY? BEACON module-list
```

When the module list is empty it is omitted along with its `+`, so a
beacon-only config starts with the beacon group as in the third example below.
A config has at most one module list, and it always comes first.

- A module list never uses coefficients: a repeated module is written as that
  many module tokens in a row. Repetition is the only spelling, so a config has
  exactly one rendering.
- A beacon group is prefixed by its beacon count, omitted when the count is
  one. The count is a coefficient because repeating a beacon token would not
  say how many modules each beacon holds.
- Beacon quality and module quality are prefixes, omitted when normal.
- Groups and lists are joined by `+` with no spaces.

```text
:efficiency-module-3::efficiency-module-3:
2:beacon::speed-module-3::speed-module-3:
:beacon::speed-module-3::speed-module-3:+2:beacon::quality-module-1:
2:legendary::beacon::quality-module-1:
```

Modules are stored as a sorted list with per-id counts, and each renders as
that many module tokens.

### 7.6 Stack limits

Stack rates and capacities print as an amount, the `:stack-size:` glyph, and a
subscript for a limited cap:

```text
3.2:stack-size:/s + 5.01:stack-size:₁/s
3.2:stack-size: + 5.01:stack-size:₁
```

`Full` prints with no subscript. Terms print `Full` first, then limits in
descending order. These renderings are for reading, not for re-parsing.

### 7.7 Processes

The **single-line result** prints the machine total, then the net rows,
separated by ` · `. A machine-seconds process marks the total with
`*s`.

The **panel** lists, one line each:

- a power line, or an energy line for a machine-seconds process;
- a pollution line, per minute for machines and total for machine-seconds;
- every row with a nonzero scale, its scale before the machine glyph —
  `288<foundry>`, or `53.333s*<foundry>` for machine-seconds — then its craft
  rate or craft total;
- an `in` header with one line per resource whose net is negative, aggregated
  over the whole process;
- an `out` header with one line per resource whose net is positive, aggregated
  the same way.

Rows whose scale is zero are omitted, and zero resource amounts disappear in the
aggregation. Units follow the extent: machines and crafts per the configured
time unit for a `Machines` process, machine-seconds and craft totals for a
`MachineSeconds` one.

### 7.8 Settings that affect display

| Setting | Effect |
|---|---|
| `max_decimals` | rounding precision (§7.2) |
| `metric_prefixes` | `k`/`M`/`G`/`T` on item and fluid amounts and rates (§7.3) |
| `time_unit` | the unit for time quantities and rates (§7.4) |
| `item_rate_belt_display` | the belt tier item rates are rendered as, or `None` for items per second |
| `stacked_belts_display` | whether those belt counts use the stacked form |
| `belt_stacking` | the belt-stacking research level, one through four; also read by the `:stacked-belt:` prefix during evaluation |
| `idle_power` | whether idle draw enters machine and process power, and whether the machine count is rounded up for it (§6.3) |

`item_rate_belt_display` and `stacked_belts_display` apply to item rates in the
`quantity` facet and in process input, output, and net rows. A rate renders as
a count, the tier's belt icon, and the item's icon, and the stacked form puts
the stack-size glyph in front of the belt icon. The `stack_rate` and
`stack_capacity` facets are already measured in stacks and are never converted.

## 8. Diagnostics

Every error carries a span and a message. An evaluation error is underlined
at the subexpression that produced it, not at the whole statement. Four kinds:

| Kind | Example |
|---|---|
| lexical | `3x` is not a number followed by a unit; a name the registry lacks |
| syntactic | `expected )`, `loop needs an integer count` |
| facet loss | `` `+` dropped every facet (left: quantity, right: machine) `` |
| semantic | `recipe is not allowed in this machine`, `division by a multi-term quantity` |

Facet-loss errors are the only place the facet inventory appears in a message,
and they name the operands' facets because that is what explains the failure.
The message also carries one reason per dropped facet, such as
`a quantity cannot have two resources in the denominator`, so an operator that
lost everything says why. A drop that depends on the values is explained — the
same pair of facet kinds combines for some values and not for others, as with
two machines whose configs differ — while a pair with no rule at all is only
named in the facet list. The reasons are collected and shown only when every
facet is gone: a partial drop is silent (P3), so no warning accompanies a value
that lost a facet and kept the rest, and the panel simply shows the facets that
remain.

A definition that fails poisons its name. Reading the name reports
`` `x` has error from line L ``, underlined at the read itself, where `L` is
the line that failed. The original message is not repeated, so the pointer is
at the use and the cause is one hop away.

An error never stops evaluation (§2.5). A cancelled evaluation (§2.7) reports
nothing at all.

## Appendix A. Operator matrix

The magnitude facets are `quantity` (Q), `stack_rate` (T), `stack_capacity`
(K), `machine` (M), `config` (C), and `process` (P). Reference facets are
dropped by every operator in this appendix except `making`, which reads
`recipe` (§5.1). Any pair not listed drops both facets; if no facet survives,
the operation is an error.

### A.1 `+` and `-`

| Pair | Result |
|---|---|
| Q, Q | add when `per` and all exponents match, with both amounts in the same case; a zero amount is the additive identity whatever shape it was written in |
| T, T | add when both are the same medium |
| K, K | add when both are the same medium |
| M, M | add when prototype, quality, and config match; counts add, absent count as 1 |
| C, C | `+` merges module and beacon counts; `-` drops the config |
| P, P | add when the extents agree and neither is `Unscaled`; an `Unspecified` process is the identity; row scales add |

### A.2 `*` and `/`

| Pair | Result |
|---|---|
| scalar, any | scales the other operand |
| Q, scalar | scales the quantity |
| Q, Q | §5.3, including `per` cancellation |
| T, Q (pure time) | `*` gives K; `/` drops, since a rate divided by time is undefined |
| K, Q (pure time) | `/` gives T; `*` drops |
| M, scalar | scales the count |
| C, scalar | scales counts; a non-integer or negative factor drops the config |
| P, scalar | scales row scales and the extent |
| P, Q (pure time) | `*` from a `Machines` process gives `MachineSeconds`; `/` from a `MachineSeconds` process gives `Machines` |

### A.3 `^`

Only Q with a number amount (§5.3), and a bundle base only with exponent `1`.
Every other pair drops, and the operator errors only when no facet survives.

### A.4 Keyword operators

`making`, `in`, `into`, and `from` are ordered pairs (§5.4). `in`, `into`, and
`from` take Q on the left and M or P on the right; `making` takes M on the left
and a recipe on the right. Every other facet on either operand drops.

## Appendix B. Acceptance examples

Each example is a document plus the invariants its results must satisfy.
Values derivable from registry constants are stated exactly; machine counts are
checked structurally, since they depend on game data and settings.

### B.1 T1 — stacked belts from foundries

```text
q = 3 :stacked-belt: :express-transport-belt: :iron-plate:
q.:iron-plate:                          # 540:iron-plate:/s
F = :foundry: (4 :speed-module-3: + 8 :beacon: 2 :speed-module-3:)
p = q from (F making :iron-plate:)
p.net.:iron-plate:                      # 540:iron-plate:/s
p.machines                              # a machine count
p.:electric:                            # watts
```

- `q.:iron-plate:` is exactly `540:iron-plate:/s`: 45 items/s for an express
  belt, times 4 for stacking level 4, times 3 belts. The 4 is the belt-stacking
  setting's default; at another level the stacked rate differs.
- `p.net.:iron-plate:` equals the requested rate. This one assertion exercises
  belt application, `:stacked-belt:`, config parsing, `making`, `from`, and
  process scaling together.
- `p.machines` is dimensionless and positive.
- `from` is correct here, not `into`: iron plates are the recipe's *output*.

### B.2 T2 — steel from a fixed input

```text
F = :electric-furnace: (2 :speed-module-3: + 4 :beacon: 2 :speed-module-3:)
p = 100 :iron-plate: into (F making :steel-plate:)
p.net.:iron-plate:                      # −100:iron-plate:
p.net.:steel-plate:                     # positive, productivity applied
p.:time:                                # the machine-seconds, as seconds
```

- `p.net.:iron-plate:` is exactly `−100:iron-plate:`, because `into` sizes the
  process so that consumption matches the request. This is the assertion that
  distinguishes `into` from `from`.
- `p.:time:` reports the extent as a plain time quantity. Machine-seconds is
  not a unit that quantities can carry: the number is the work done, and the
  wall-clock duration is that number over however many furnaces you run.
- `.machines` is not available here. It is a property of a rate process, and
  this process is a count process.
- Changing the research level changes `p.net.:steel-plate:` and leaves
  `p.net.:iron-plate:` untouched.

### B.3 T3 — quality cycling

```text
R = :recycler: (4 :quality-module-2:)
q = :recycling: :iron-plate:
p = 1000 :iron-plate: into (R making q)
loop 1000 {
  p += p.out.:iron-plate: < :rare: into (R making q)
}
p.out.:iron-plate: >= :rare:
```

- The recycling recipe has to be named with `:recycling:`; it is hidden, so it
  is not offered by autocomplete. It stays resolvable by name and inferable by
  `into`.
- Inner `into` scales one row per resource term: `p.out.:iron-plate: < :rare:`
  has a normal term and an uncommon term, which select two of the recipe's
  quality rows, so no double count is reported and the unselected rows are
  dropped.
- In an iteration where no term is below rare, the filter yields `0`, sizing
  yields a zero process, and `p +=` is a no-op. `p` is never poisoned.
- `p` stays a `MachineSeconds` process across iterations, so `+=` is valid.
- Because `p` is a `MachineSeconds` process, `.machines` does not apply; T3's
  machine count is read by dividing the process by a time quantity (§5.4),
  `(p / 1 s).machines`.
- The final selector returns the rare-and-better output only.

### B.4 Renderings round-trip

```text
a = :foundry: (2:speed-module-3:)
b = :foundry: :speed-module-3::speed-module-3:
```

`a` and `b` denote the same machine by structural equality (§3.5): both carry a
config with two speed modules. There is no equality operator, so the two
bindings are compared by inspecting their structure, not by an expression
(§5.3).

A rendering pasted back into the document must produce a value equal under that
equality for quantities, configs, machines, and qualities (§7.5).

## Appendix C. Decision log

| ID | Decision | Rationale |
|---|---|---|
| D1 | Operator failure policy: lenient and silent; an operator errors only when every facet is lost. | P3. Keeps facets independent and keeps output quiet; reporting drops would be noise, since dropping is the normal case. |
| D2 | Unscaled prototype references are facets of a single value, not a distinct top-level variant. | A token is simultaneously an item, a recipe, and a machine; one value must carry all. |
| D3 | A single `resource` facet, not sibling `item` and `fluid` facets. | Makes item/fluid exclusivity structural. Item-only sites use `.item()` / `.fluid()` helpers. |
| D4 | Redundant facets are retained rather than derived on demand. | P1. Operators never reach across facets. |
| D5 | Quality: concrete in quantity terms, optional in reference facets. | P2, §3.2. Enables `v.:iron-plate:` to span qualities. |
| D6 | The grammar is complete before the value model is. | P4. Precedence is cheap to fix now and expensive later, so the grammar is complete from the start while the value model grows facet by facet. |
| D7 | Deliverables: this specification, an operator matrix, and an executable example table. | Implementation target plus regression corpus. |
| D8 | The quality operand rewrites all item terms including the denominator, leaves non-item terms alone, is a no-op with no items, and is an error on a process. | §3.3. Process quality is machine- and recipe-determined, so relabeling it would be a lie. |
| D9 | The quality operand rewrites the identity a value names, per facet: every quantity item term, the resource and recipe references, a machine's prototype, and a config only when it names one identity with count one — a single module of count one, or a single beacon of count one, whose beacon is promoted and whose modules are untouched. | P1. Rewriting the quantity while leaving identity behind would leave the two disagreeing. The count-one restriction exists because a qualified token promotes only its own prototype, so promoting a counted or repeated entry from the value form would mean something different from the token form. |
| D10 | A definition is a statement whose left side is a place, not an operator. | Expressions stay effectless; `=` needs no precedence and cannot nest. |
| D11 | A failed definition poisons the name. | A stale binding is worse than a propagated error. |
| D12 | `loop N { … }` is the only block form, with a flat global environment. | T3 needs iteration; the body has no closed form in the language. |
| D13 | A quality token is a value; a quality token directly followed by a prototype token is one qualified token. | Qualities must be storable and reusable, and a quality before an icon must apply to that icon regardless of what is on its left. Lexing both cases avoids a context-sensitive grammar. |
| D14 | `MachineMakingRecipe` is folded into `Process` as extent `Unscaled`. | It is the process map key, not a value: two representations of "machine plus recipe, not yet sized" must not coexist. |
| D15 | No error stops evaluation, including inside a loop body. | A notebook, not a transaction: independent statements must not block each other. |
| D16 | Evaluation runs off the UI thread and is cancelled at every statement boundary, including each loop iteration. | A large or mistyped loop count must not freeze the application. |
| D17 | Quality values expose `.level` and `.multiplier` only. | `level` is the registry field; `multiplier` is `1 + 0.3 × level`, derived because the registry has no single multiplier field. |
| D18 | There is no `with` operator; a config is attached to a machine by juxtaposition, machine on the left. | One fewer keyword and one fewer precedence level. `:foundry: 2 :efficiency-module-3:` reads better than `:foundry: with 2 :efficiency-module-3:`. |
| D19 | Quality names accept `:common:` and `:quality-N:` as aliases, and autocomplete offers them. | `normal` is easy to mistake for "no quality"; `q1`…`q5` are the natural thing to type. |
| D20 | `-` binds tighter than `+` and `/` tighter than `*`. | The `/` inversion is required for readable ratios; the `-` inversion follows it for symmetry, and is unobservable because a `Quantity` is an additive bundle so grouping cannot change a result. |
| D21 | Juxtaposition is right-associative; there is no number-before-token production. | Makes `:beacon: n :speed-module-3:` agree with `:beacon: 2 :speed-module-3:`. Accepted cost: `2 :speed-module-2: 3 :productivity-module-2:` doubles the productivity count (§4.1). |
| D22 | Config juxtaposition needs no rule about item facets; they drop by P3 because `item × item` has no rule. | A rule to retain the left item would be a special case, and it is what produced the beacon bug. |
| D23 | The single-line result shows the first facet; the panel shows every facet. | The results column is fixed width and the projection is already defined as lossy; the panel is where the rest are read. |
| D24 | Round to `max_decimals` fraction digits at or above 1 and significant digits below 1. | Counts stay exact, and small values do not collapse to `0` before the `1e-9` threshold is reached. |
| D25 | Quantities, qualities, configs, and machines render round-trippably; stack limits do not. | A result can be pasted back into the document to check it. Subscripts would need parser support for no benefit. |
| D26 | Time applies to rates too: `15/s` renders as `900/m` in minute mode. | The configured unit is what the user reads; mixing `m` for durations and `/s` for rates is inconsistent. |
| D27 | `item_rate_belt_display` and `stacked_belts_display` are display settings. | Item rates in the `quantity` facet and in process rows are items per second, and rendering them as belts is the point of those settings. |
| D28 | Scaling drops the `resource` facet, so `:stack-size: (3:iron-plate:)` is an error. | "Stack of three iron plates" is not what the phrase means; `3 :stack-size: :iron-plate:` is the correct grouping and already works. |
| D29 | Configs render with no coefficients on modules, beacon-group counts as coefficients, groups joined by `+` with no spaces, and a machine's config always in parentheses. | Repetition gives a config exactly one spelling; the beacon count has no repetition form, since repeating a beacon would not say how many modules each holds. The parentheses keep `+` inside the config instead of turning it into an addition to the machine. |
| D30 | `:common:` renders in reference facets and in quality values, but not in quantity terms. | A reference facet distinguishes `Some(Common)` from `None`; a quantity term always has a concrete quality, so normal is the default there. |
| D31 | A quantity cannot represent a product of bundles, two resources in the denominator, or a bundle raised to a power. Those operations drop the `quantity` facet, and the operator errors only when nothing survives. | Deliberate trade: the shape is closed under addition and subtraction, which the old item monomial was not — `2:iron-plate: + 3:copper-plate:` dropped the item facet before and is a first-class value now. Accepted loss: `(2:iron-plate:)*(3:copper-plate:)` and `(2:iron-plate:)^2`. |
| D32 | Multiplying by a single-term bundle cancels a matching `per`. | Without cancellation the ratio idiom that motivates `/` binding tighter than `*` cannot be evaluated at all. |
| D33 | A quality filter keeps the item terms that satisfy it and drops everything else, fluids included. | A fluid has no quality, so it cannot satisfy a comparison; dropping is the P3 behaviour, and `.:fluid:` recovers the fluid part. |
| D34 | A process stores its net rows — what the recipes produce minus what they consume, including assignment — alongside each row's scale; per-row rates still come from the machine, config, and recipe. | Assignment edits the stored total in place, and `.net`, `.in`, and `.out` are read from it; the document is re-evaluated when a setting changes rather than keeping a stale net. |
| D35 | Sizing with a zero magnitude yields a zero process with no rows, not an error. | T3's loop reaches `0` and must add nothing rather than poison the accumulator; nothing needs to be selected, so no recipe is resolved. |
| D36 | A config may hold several beacon types, and qualities of one beacon are distinct types. | The game permits it and the distribution profile depends only on the total beacon count; the old restriction existed only because the old `Config` could not represent it. |
| D37 | `.:item:` and `.:fluid:` select by medium. | A quality filter drops fluids, so there must be a way to take the fluid half of a mixed quantity. |
| D38 | `Unscaled` and `Machines` processes report rates; `MachineSeconds` reports totals; a property with no meaning for its extent is an error. | Extent is the honest distinction: a sized machine count produces continuously, machine-seconds are finished work. |
| D39 | `.productivity` returns the multiplier `1 + bonus`. | It is what a caller multiplies output by; the bonus is one step short of useful. |
| D40 | `.:electric:` is kept and selects power or energy by what the operand has; `.:time:` is kept for count processes. | `.:electric:` is short and extent-independent, which is what the UI needs. `.:time:` is the only way to read a count process's extent as a time. |
| D41 | A quantity with several resource terms scales one process row per term, and two terms selecting the same recipe is an error. | T3 filters to a set of qualities, and each quality is its own recipe; the same recipe twice would double count. |
| D42 | There is no `belt` facet. | No operator reads one. A belt token carries `quantity`, `stack_rate`, and `stack_capacity`, and `:stacked-belt:` works on any non-fluid `stack_rate`. |
| D43 | Machine-seconds is an extent magnitude, not a unit; `.machines` is a rate-process property and `.:time:` is a count-process property. | Quantities carry only `second`, `joule`, `craft`, and resources; the extent already says whether the number is a rate or a total. |
| D44 | Hidden items and hidden recipes are excluded from autocomplete but not from inference, and remain reachable by naming them (`:iron-plate-recycling:`). | A recycler mostly makes hidden recipes, so excluding them from inference makes `into :recycler:` fail; they are kept out of the completion list because they would dominate it. |
| D45 | `:stacked-belt:` requires a single `Limit(1)` stack limit and errors otherwise. | Stacking is a belt operation. An inserter's rate is incidentally at `Limit(1)` before capacity research, which is meaningless but not worth a special case to reject. |
| D46 | Floating-point precision is treated as irrelevant: arithmetic is assumed to be algebraic, and differences below the `1e-9` threshold are not observable. | The calculator is a planning tool; carrying rounding semantics through every operator would cost more than it explains. |
| D47 | Quality filters bind tighter than arithmetic. | They behave like suffixes on a quantity rather than comparisons of magnitude, so `value < :legendary: > :normal` reads as one filtered value. |
| D48 | Sizing by zero yields `Unspecified` so that `a p1 + b p2` always works for unscaled processes `p1` and `p2` and dimensionless scalars `a` and `b`, even when either scalar is zero. | The empty process must be the identity under `+`, or a zero term would poison an accumulator. |
| D49 | The approximate-zero threshold is `1e-9`, chosen arbitrarily. | One part per billion is far below any factory rate and far above float noise. |
