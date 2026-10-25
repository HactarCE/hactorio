## jj workflow

Use `jj`, never `git`.

Before starting a unit of work, run `jj status`.

- If the description is unrelated to current work, run `jj new`.
- If the description is related to current work, continue on the current commit.
- If the working copy is already empty and has no description, use it; do not run `jj new` again.
- To amend an earlier commit, run `jj edit <change>`; descendants rebase automatically.

After completing a unit of work, run `jj describe` and describe the whole set of changes, including the existing work on the commit (if any) and the new work you just did. Run `jj new` before the next unrelated unit: `jj describe` does not close the commit, so later edits amend it.

## README

`README.md` is the user's. Do not edit it — report proposed wording instead.

## UI vocabulary

A floating surface is named by what it describes and how it presents. Keep
these names in code, comments, and docs:

| Name | Meaning |
| --- | --- |
| results column | the fixed column of per-line results right of the editor |
| single-line result | the lossy, one-line projection painted in a results-column row |
| multiline result panel | the floating panel with a line's full result; follows the caret |
| token tip | the tooltip for the prototype or variable under the pointer; follows the pointer |
| hint | stock `on_hover_text` text on a control |

`tip` always means an `egui::Tooltip`; `panel` always means a floating
`egui::Area`. "Popup" means the completion dropdown only. Do not call the
results column a gutter or a panel: line numbers are the left strip, results are
the results column.

A private-use character is a **glyph**; the PNG it renders into is an **icon**.
So `:iron-plate:` is a glyph, its entry's `icon` path points at an icon, and
`GLYPH_SIZE` is how large the character is drawn. A PUA glyph is not
necessarily the same width as other characters in the monospace font, so do not
rely on column alignment between glyphs and text.

A tip only opens when it has something to describe: hovering a keyword, a typo,
or a variable that does not resolve shows nothing.

## egui

Rows inside content-sized panels use `ui.horizontal`; `ui.horizontal_top` allocates the remaining height and causes a growth feedback loop.

## Factorio data

The foundry cannot run the `iron-gear-wheel` recipe (category `crafting`), but it has `casting-iron-gear-wheel` (category `metallurgy`), which also produces iron gear wheels. Recipe resolution in `in` must filter by the machine's categories; `:iron-gear-wheel: in :foundry:` is valid and resolves to the casting recipe.

Inserter `hand_size` is cosmetic; capacity comes from `bulk`, `stack_size_bonus`, and the inserter capacity research modifiers.
