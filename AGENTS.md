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

A tip only opens when it has something to describe: hovering a keyword, a typo,
or a variable that does not resolve shows nothing.

## Text format

Documents store game icons as plaintext ASCII tokens delimited by colons —
`:iron-plate:`, `:time:` — so saved text never depends on the codepoint table.
In the UI, `from_plaintext` substitutes each known token with a private-use
codepoint and the rasterizer draws it from the entry's icon; `to_plaintext`
converts back when saving.

A private-use character is a **glyph**; the PNG it renders into is an **icon**.
So the `:iron-plate:` token is the plaintext spelling of a glyph, its entry's
`icon` path points at an icon.

A glyph is square and exactly one font size wide. When the icon file is a mip
strip (wider than tall) only its left full-size square is kept, and that square
is scaled to `round(font_size × pixels_per_point)` pixels. The advance is
`font_size` points: the rasterizer returns `advance_px = font_size_px` and epaint
divides that by `pixels_per_point`, so the icon's size never affects layout.
`GLYPH_SIZE` is the glyph font size in the display-settings buttons; the editor,
results column, completion popup, and tips draw glyphs at the monospace font
size.

A monospace character is narrower than the font size, so a glyph is not one text
column wide and glyphs and text do not share column alignment. Do not align
columns across them; size a glyph column as `count × font_size` points.

## egui

Rows inside content-sized panels use `ui.horizontal`; `ui.horizontal_top` allocates the remaining height and causes a growth feedback loop.

## Factorio data

The foundry cannot run the `iron-gear-wheel` recipe (category `crafting`), but it has `casting-iron-gear-wheel` (category `metallurgy`), which also produces iron gear wheels. Recipe resolution in `in` must filter by the machine's categories; `:iron-gear-wheel: in :foundry:` is valid and resolves to the casting recipe.

Inserter `hand_size` is cosmetic; capacity comes from `bulk`, `stack_size_bonus`, and the inserter capacity research modifiers.
