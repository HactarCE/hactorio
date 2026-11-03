I want to redesign this Factorio calculator.

Traditional Factorio calculators evaluate the whole recipe chain. You start by saying "I want 10 advanced circuits per second" and they compute all the intermediate machines you need. I don't like this because it solves too narrow of a problem.

This calculator is more of a notebook-style calculator like Numara, where you type expressions into a textbox and the program evaluates them line-by-line.

Example questions I want to be able to answer using this calculator:

- I want 3 stacked blue belts of iron plates; how many foundries (with some specific module/beacon configuration) do I need?
- How much steel can I make using 100 iron plates, using electric furnaces (with some specific module/beacon configuration, and some specific steel productivity research)? How many seconds will it take?
- Assume I have 1k iron plates, and I recycle them repeatedly (with some specific module/beacon configuration) until they are rare quality or better. How many recyclers do I need, and how many rare iron plates will I get?

Each of the components is built from composable subexpressions. E.g., `3 :stacked-belt: :express-transport-belt: :iron-plate:` parses as `3 ((:stacked-belt: (:express-transport-belt:)) :iron-plate:)` -> 3 stacked blue belts of iron plates.

## Facets

A crucial concept here is that one value may have multiple **facets** capturing different aspects. For example, `:transport-belt:` is an **item** (really a **quantity** of 1 item), a **rate** (it can transport 15 item stack/s), a **capacity** (it can hold 8 item stacks at a time), and a **recipe** (for producing transport belts). Below are all of the possible facets:

- **Quantity**, a restricted polynomial of items and other base units
- **Rate** (either **Item Rate** or **Fluid Rate**)
- **Capacity** (either **Item Capacity** or **Fluid Capacity**)
- **Machine**
- **Config**
- **Process** (either **Rate Process** or **Count Process**)

### Quantity

A Quantity is represented using a struct with the following fields:

- `amount` — plain number, or coefficient per resource
- `per` — optional resource in the denominator
- `second` — exponent for "second" base unit
- `joule` — exponent for "joule" base unit
- `craft` — exponent for "craft" base unit

### Rate/Capacity

Rates and Capacities differ only in units: rates represent items or fluid per second, while capacities represent a fixed amount of items or fluid. Item rates/capacities are represented using a `BTreeMap<StackLimit, f64>`, with `struct StackLimit(u32)`. Fluid rates/capacities are represented using a single `f64`.

Display examples:

- Item rate: `3.2:stack-size:/s + 5.01:stack-size:₁/s`
- Item capacity: `3.2:stack-size: + 5.01:stack-size:₁`
- Fluid rate: `15:fluid:/s`
- Fluid capacity: `15:fluid:`

#### `StackLimit`

- `StackLimit(u32::MAX)` represents the limit imposed by a steel chest (i.e., simply `min(u32::MAX, item.stack_size) = item.stack_size`)
- `StackLimit(4)` represents the limit imposed by a stacked belt with all belt-stacking researches unlocked. (i.e., `min(4, item.stack_size)`)
- `StackLimit(1)` represents the limit imposed by a non-stacked belt
- `StackLimit(0)` is invalid and never appears

In general, `StackLimit` is displayed using the `:stack-size:` icon followed by subscript digits. There is, however, one special case: `StackLimit(u32::MAX)` is displayed using `:stack-size:` with no subscript

### Machine

A Machine consists of an entity prototype (e.g., assembly machine 1, foundry, steel furnace) and a Config, which may be empty.

### Config

A Config consists of a list of modules and a list of beacon configurations (`BeaconConfig`). The beacon configurations are stored in a `BTreeMap<BeaconConfig, u32>`. Each `BeaconConfig` consists of a module list.

Only the standard vanilla beacons are allowed.

## Primitives

### Number literals

A number literal is a Quantity. Number literals may have a suffix, consisting of an optional metric prefix (only `k`, `M`, `G`, `T` are accepted) and an optional unit abbreviation (only `s`, `m`, `h`, )

### Prototype

A prototype

### Item/fluid/recipe icon

Every non-hidden item/fluid/recipe has an associated icon. When appearing in expression position, these icons have the following facets:

- Quantity
- Item (if applicable)
- Fluid (if applicable)
- Recipe (if applicable)
- Machine (if applicable)
- Config (if module/beacon)

## Operators

### Prefix operators

#### `-`

Negation works on any facet that is capable of being negated. For Quantity facets, all values in the numerator (`amount`) are negated. For Process facets, only the craft rate/count is negated.

#### `:stacked-belt:`

#### `:stack-size:`

#### `:recycling:`

This prefix takes an Item and returns the Recipe for recycling that item. This cna be found by finding the single recipe in the recycler that takes that item as input. If such a recipe does not exist, then it errors.

### Juxtaposition

Juxtaposition is the tightest binary operator and hardly feels like an operator: it's more like forming a "word" in the language.

Juxtaposition has the following valid forms:

- `SpaceLocation(None)` with `Planet(planet_id)` (commutative) evaluates to a `SpaceLocation(Some(planet_id))`
- Location with SolarPanel (commutative) evaluates to a Quantity of power production during the day

### `.property`

Unary suffix operator that access a property of a value. The following properties are allowed:

- `.rate` — Returns only the Rate from a value and removes all other facets
- `.capacity` — Returns only the Capacity from a value and removes all other facets
- `.power` — Returns the signed power output of an Entity (e.g., solar panel, beacon), Machine (e.g., foundry), or Rate Process
- `.energy` — Returns the signed energy output of a Count Process as a Quantity with the appropriate units, or the energy capacity of an Entity (e.g., accumulator).
- `.machines` — Returns the number of machines in a Rate Process
- `.crafts` — Returns the number of crafts in a Count Process
- `.productivity`/`.prod` — Returns the productivity multiplier for a Process
- `.pollution` — Returns the pollution for a Process (units: 1/s for a Rate Process, or dimensionless for a Count Process)
- `.abs` — Returns the absolute value of almost any facet (does not work on Process facet and facets that cannot be negated)
- `.:stack-size:` — Returns the stack size of an Item

Properties produce an error if they are invalid (either "unknown property name" or "expected <some_facet>, got <facet_list>").

Parsing note: if `.` is followed immediately by alphabetic characters (regex `\.[a-zA-Z]+`), it is _always_ parsed as a property. E.g., `x.rate` and `x .rate` are both always the rate accessor, but `x. rate` and `x.(rate)` are both invoctions of the binary operator `.` with the value of the variable `rate`. `x.invalidproperty` always errors becase `invalidproperty` is not one of the property names listed above.

## Order of operations

From tighest- to loosest-binding:

1. Unary prefix operators: `-`, `:stacked-belt:`, `:stack-size:`, `:recycling:`
2. Juxtaposition
3. Unary suffix operator: `.property`
4. `.` (binary)
5. Quality filter operators: `<`, `>`, `<=`, `>=`, `==`, `!=`
6. `^`
7. `/`
8. `*`
9. `-` (binary)
10. `+`
11. Machine configuration operator: `with`
12. Recipe operator: `making`
13. Process scaling operators: `into`/`from`/`in`
14. Assignment operator: `=`

All operators are left-associative except for `^`, which is right-associative.

Reason for quality filter operators being tighter than other binary operators: these operators are not being used as traditional comparison operators, but as postfix filters. They should be looser than binary `.` (so that `process.:iron-plate: < :rare:` works) but tighter than arithmetic operators (so that they can be used to form larger expressions, such as `process.:iron-plate: < :rare: + process.:copper-plate: < :uncommon:).

Reason for `/` being tighter than `*`: I want to allow item ratio calculations like `3:iron-plate: * 1:gear:/2:iron-plate:`. Parsing left-to-right using traditional order-of-operations, we get `(3:iron-plate: * 1:gear:)/2:iron-plate:`, which errors because "iron plate times gear wheel" is unrepresentable. But with `/` binding tighter than `*`, we get `3:iron-plate: * (1:gear:/2:iron-plate:)` which is ok. Algebraically, making `/` bind slightly tighter than `*` makes no difference: `a*b/c*d = ((a*b)/c)*d = a*(b/c)*d`. For consistency, `-` binds tighter than `+`.

Reason for keyword precedences: `with` should bind looser than `+` because `+` is used to combine machine configurations and `with` consumes machine configurations. `making` should bind looser than `with` because it consumes machine configurations. Similarly, `into`/`from`/`in` should bind looser than `making`.

## Misc

### Comparison to zero

All comparisons to zero use approximate comparison.

### NaN/inf

NaN and infinities are allowed in operations, and are exposed to the user.

## Things I'm not sure about

What should be considered facets? We could have things like "beacon" and "belt" but these _could_ be inferred from the "item" facet. Should (unscaled) "item", "fluid", and "recipe" be separate, or just one combined facet representing an item/fluid/recipe that has not been scaled or used in arithmetic?

## Uncategorized/unprocessed

new syntax for process scaling:
- `Polynomial into MachineMaking` or `Polynomial into Machine` (input, infers recipe)
- `Polynomial from MachineMaking` or `Polynomial from Machine` (output, infers recipe)
- `Polynomial in MachineMaking` (net)


r = 1:iron-plate:/s
p * r/p.(r*s)

new syntax for filtering:
- `<QUALITY`
- `>QUALITY`
- `<=QUALITY`
- `>=QUALITY`
- `==QUALITY`
- `!=QUALITY`

new syntax for looping:
```
loop 1000 {
  p += p.out<:rare: into :recycler:
}
```

e.g.,

```
p = :scrap: into :recycler:
```



s
J
W
(item & fluid) /s
s / (item & fluid)
(item & fluid) / J
J / (item & fluid)
(item & fluid) / W
W / (item & fluid)

numerator:
- Vec<Item or Fluid>
- seconds: bool
- joules: bool

denominator:
- Option<Item or Fluid>
- seconds: bool
- joules: bool
