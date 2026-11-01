# Future ideas

## Machine

Machine can be unscaled, scaled to a number of machine-seconds, or scaled to a number of machines.

## Process

Similar to Machine, Process can be unscaled, scaled to a number of crafts, or scaled to a number of crafts per second.

Every Machine also has a Process facet containing all recipes supported by that machine, at all qualities.

Process consists of a Machine, a recipe list, and a scale (unscaled, number of crafts, or number of crafts/s). In practice, the recipe list has an enum that's either a (presumably short) list of recipes or an `Arc<BakedRecipeList>` where `BakedRecipeList` has a precomputed hashset of recipes (for `making`) and a map from input & output item/fluid to recipe (for `into`/`from` respectively, and for `in`). `BakedRecipeList` should be precomputed for each machine. (TODO: is `BakedRecipeList` a good name? Is there a better one?)

`PROCESS making RECIPE` -> restricts a Process according to selected recipe.

For `PROCESS making RECIPE`:
- if the recipe only has fluid input(s), then the recipe must not have quality specified for it.
- if the recipe has any item input(s) and has unspecified quality, then the Process contains 5 recipes (one for each quality)
- if the recipe has any item input(s) and has a specified quality, then the Process contains only 1 recipe (the one for that quality)

### Operations

- Scaled Process can be multiplied/divided by a dimensionless scalar.
- crafts/s Process can be multilpied by seconds (or divided by 1/s) to get crafts Process
- crafts Process can be divided by seconds (or multiplied by 1/s) to get crafts/s Process
- Processes can be added together if they are both unscaled or both scaled using the same units (TODO: somehow keep track of which machine configurations are doing which recipes)

## Process scaling and selection via inputs/outputs

("Item" in this section really means "resource" which may be item or fluid.)

three new binary operators:
- `into` -- selects recipe + scales based on input
- `from` -- selects recipe + scales based on output
- `in` -- selects recipe + scales based on net output (output minus input)

syntax (same for all three): `QUANTITY into UNSCALED_PROCESS`
- if `QUANTITY` has units items/s, returns a scaled crafts/s process
- if `QUANTITY` has units items, returns a scaled crafts process

If the Quantity is a sum of multiple terms, each term scales a recipe. In general we could solve a system of linear equations (which may be over/underconstrained) but this isn't worth the complexity; just process each item one at a time, find the one recipe it matches (error if ambiguous), scale that one recipe, and add it to the result. Error if two items use use the same recipe.

## Configurations



## Solar power

new facet: either Location or SolarSetting

```rs
enum Location {
    Planet(EntryId),
    SpaceLocation(Option<EntryId>),
}

struct SolarSetting {
    location: Location,
    day_only: bool,
}
```

construct Location using planet icon, space location icon, or `:space-platform:` + planet icon (commutative juxtaposition).

construct SolarSetting using Location + `:day:` icon (commutative juxtaposition)

juxtapose Location or SolarSetting with solar panel entity to get power output. if `day_only: true`, then return daytime power output; otherwise multiply by `0.7` if there's a day-night cycle to get average power output.

consider also allowing juxtaposing with an accumulator to get the power that a single accumulator can sustain during the night (min between `accumulator.max_power_output` and `0.24 * accumulator.capacity / location.day_night_cycle_length`)
