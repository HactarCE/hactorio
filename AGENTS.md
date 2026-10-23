## jj workflow

Use `jj`, never `git`.

Before starting a unit of work, run `jj status`.

- If the description is unrelated to current work, run `jj new`.
- If the description is related to current work, continue on the current commit.
- If the working copy is already empty and has no description, use it; do not run `jj new` again.
- To amend an earlier commit, run `jj edit <change>`; descendants rebase automatically.

After completing a unit of work, run `jj describe` and describe the whole set of changes, including the existing work on the commit (if any) and the new work you just did.

## egui

Rows inside content-sized panels use `ui.horizontal`; `ui.horizontal_top` allocates the remaining height and causes a growth feedback loop.

## Factorio data

The foundry cannot run the `iron-gear-wheel` recipe (category `crafting`), but it has `casting-iron-gear-wheel` (category `metallurgy`), which also produces iron gear wheels. Recipe resolution in `in` must filter by the machine's categories; `:iron-gear-wheel: in :foundry:` is valid and resolves to the casting recipe.

Inserter `hand_size` is cosmetic; capacity comes from `bulk`, `stack_size_bonus`, and the inserter capacity research modifiers.
