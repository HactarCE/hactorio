# Awaiting decisions

These are implementation-level ambiguities not fully pinned by the design doc.
The listed default is what the implementation will assume until you say
otherwise. None blocks starting the facet model/parser.

## 1. Plain config merge arithmetic

Rule 3 says a plain config merge (no attach) consumes both item facets and
returns config-only. It does not state how the two configs combine.

- **Default:** use the same arithmetic as `+`: add module counts by id; add
  beacon-group counts for groups with the same beacon and module set; keep
  different beacon/module sets separate; canonicalize.
- Alternative: concatenate groups without adding counts.

Example: `2 :speed-module-3: 3 :speed-module-3:` → config with 5 modules.

## 2. `in` LHS with extra facets

§4.6 says `q` contributes only its item/fluid/craft facet and other facets are
dropped, but also calls `q` “pure”. Does `:speed-module-3: in m` (item+config)
count as a pure item amount, or is it an error?

- **Default:** accept any value whose item facet is a pure amount or rate, drop
  all other facets (config, belt, capacity, recipe), then resolve.
- Alternative: require the LHS to have only an item facet, so extra facets are
  errors.

## 3. `0` with a negative exponent

The doc now says `0 ^ 0` is an error and negative bases need an exact integer
exponent; it does not say what `0 ^ -1` does.

- **Default:** error for `0` raised to a negative exponent.
- Alternative: follow scalar division and produce `inf`/`NaN`.

## 4. `making` RHS with extra facets

`m making r` requires `r` to have a recipe facet. Can `r` also have an item,
machine, or config facet (for example a machine token with a same-id recipe)?

- **Default:** accept any value with a recipe facet; the RHS recipe is consumed
  and all other RHS facets are dropped.
- Alternative: require a recipe-only value or item+recipe token.

## 5. `:stack-size:` item mode with extra facets

Item mode says the operand must be a pure item quantity, but the prefix drops
recipe/machine/process/config/rate/capacity facets. Is `I+C` or `I+T` a valid
operand whose extra facets are dropped?

- **Default:** accept a pure item facet and drop the other facets; a value with
  no item facet (other than belt mode) errors.
- Alternative: require an item-only value.

## 6. `^` exponent operand with extra facets

The exponent must be dimensionless; config/recipe/machine facets on the
exponent are not explicitly addressed.

- **Default:** use only the exponent’s item facet; require it to be
  dimensionless; drop the other exponent facets.
- Alternative: error if the exponent has any non-item facet.

## 7. Config division error scope

We chose “any `/` with a config facet on either side is an explicit error,”
rather than silently dropping the config facet.

- **Default:** error for the whole operation, including `I+C / 2`.
- Alternative: drop the config facet and continue with the remaining facets.

## 8. `in` LHS rate/capacity forms

A pure item rate is an item monomial with `second^-1`; a rate facet is the
separate `RateFacet`. Should `in` accept both?

- **Default:** accept a pure item/fluid amount (including a monomial rate) and
  an isolated rate facet; reject mixed item+rate/capacity facets unless the
  extra facets are droppable under decision 2.
- Alternative: accept only item-monimal rates, never rate/capacity facets.
