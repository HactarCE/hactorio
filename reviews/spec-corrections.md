# SPEC_corrected.md — disposition of `reviews/spec-validation.md` findings

Every finding in the validation review appears once below. All changes are in
`SPEC_corrected.md`; findings requiring a design decision were left alone and
remain in `reviews/spec-validation.md`.

## Fixed

| Finding | Change |
|---|---|
| §5 Blocker `6 / (2:iron-plate:)` contradicts the rule two lines above it | Changed the worked example to `3/:iron-plate:`, matching §5.3's single-term-bundle rule; the rule is unchanged. |
| §2.2 "the specification marks assignable" marks nothing | §2.2 now names the marked selector: a process's resource row. |
| §3.3 qualified values and unqualified selectors disagree | §3.3 now says a qualified value fails a selection at a different quality; an unqualified selector still matches every quality. |
| §3.3's config rule rejects §4.2's own example | Removed the empty-module-list condition in §3.3 and D9, so a single beacon promotes with its modules untouched. |
| §4.2 "every operator rejects a quality value" vs juxtaposition | §4.2 now exempts juxtaposition alongside the two properties and the filters. |
| §4.4 the adjacency rule forbids the language's own canonical renderings | §4.4 now allows a number followed by a unit keyword or a `:token:`. |
| §5.2 contradicts itself on unmapped facet pairs | §5.2 now says an unmapped facet pair drops, matching P3 and its own examples. |
| §5.4 two answers for sizing by zero | Zero sizing now yields extent `Unspecified`; the `Extent` comment lists that variant. |
| §5.4 `+` extent rule vs the `Unspecified` identity | `Unspecified` is now stated to be the identity under `+`. |
| §5.4 `Unscaled` arithmetic: two rules, opposite results | A.1's `P, P` row now excludes `Unscaled`, matching §5.4. |
| Appendix A.1 admits arithmetic that §5.4 forbids | The `P, P` row now requires both extents to agree and be non-`Unscaled`, and records the `Unspecified` identity. |
| Appendix A.2 omits rules §5.4 requires, and is asymmetric | Added `Q, scalar` and `P, Q (pure time)` rows. |
| Appendix A.4 describes the keyword operators wrongly, and A's preamble contradicts §5.1 | A.4 now separates `in`/`into`/`from` from `making`, and the preamble excepts `making`'s `recipe`. |
| Appendix B.3's invariant contradicts the extent rule | B.3 now says `p` stays a `MachineSeconds` process. |
| §7 "Display never affects evaluation" is false | §7 now excepts `belt_stacking`, which `:stacked-belt:` reads. |
| Appendix C D24 says "fraction digits above 1" | D24 now says at or above 1, matching §7.2. |
| Appendix C D25 lists "Quantities, configs, and machines" | D25 now includes qualities. |
| Appendix C D30 is narrower than §7.5 | D30 now says `:common:` renders in reference facets and quality values. |

## Left

| Finding | Why no obvious answer |
|---|---|
| §0 T3's machine-count half has no defined answer | Changing B.3 to a rate or adding a `.machines` step would decide what T3's example means. |
| §1 P3 vs §5.3's unconditional quantity errors | Whether two `per`s, bundle×bundle, or multi-term division error against P3 is a policy choice. |
| §1 P4 vs §6.6 | Whether facet selectors such as `.power` are syntax or properties is a design choice. |
| §1 P5 vs §6.3 | A process-level carve-out for the quality distribution over outcomes is a design decision. |
| §2.2 "stored adjustment" has no representation | Requires a per-resource adjustment field or a different assignment rule. |
| §2.2 compound-assignment operand types | `+=`/`-=` domains and assignment to `Unscaled`/`Unspecified` processes need a decision. |
| §2.1 no rule for malformed blocks | Recovery for a stray `}`, an unclosed `{`, or a bad loop header is a parser policy choice. |
| §2.1 comment vs token | Whether `#` may occur inside a token, and whether a comment may follow `{`, is undecided lexical policy. |
| §3.3 quality × quality is undefined | Error versus no-op for `:rare: :epic:` is a design choice. |
| §4.3 levels 3/4 cannot be composed | Requires choosing a precedence or grouping rule for `.property` and `.` with a resource. |
| §4.3 the justification for level 4 is void | Needs a new rationale for the level, not a correction of a citation. |
| §4.3 unary `-` at level 7 makes `^` and filter right-hand sides unparseable | Fixing needs a precedence or grammar change with other consequences. |
| §4.4 `.5` vs the `.` operator | Needs a maximal-munch tie-break rule. |
| §4.4 `1MJ` has two readings | Needs a lexer rule choice; the values coincide. |
| §4.3 signal selectors have no precedence level | Requires assigning a level, which decides `p.:item:.abs`. |
| §4.3 level 5 is misnamed and inverted without justification | Needs a precedence redesign, not a wording fix. |
| §4.3 left-associative filters accept `a < b < c` | Whether chained filters are legal is unstated by design. |
| §5 "the applied amount" is the whole rate→item conversion and is undefined | Needs the game-data conversion formula. |
| §5 "config that is one beacon and nothing else" is ambiguous | One group versus count one is a design choice. |
| §5 is an empty module list a module list? | Whether the empty list participates in the merge rule is a design choice. |
| §5 non-integer config scaling: drop or error? | P3 versus §5.5 is a policy choice. |
| §5 "the only common one" does not choose a quality | The tie-break is unstated by design. |
| §5 `-` and `.abs` are defined only for quantity and config | Defining `T`, `K`, `M`, `P` behaviour is a design choice. |
| §5 `^` coverage and drop-vs-error | Needs a rule choice between dropping and erroring. |
| §5 self-cancelling division | Cancel, `per`, or error is a design choice. |
| §5 §5.1's table misuses its own columns | Rewording depends on whether `.` reads a reference facet or matches terms. |
| §6 `craft` is simultaneously an exponent and a row | Needs a value-model decision for craft terms and `.crafts`. |
| §6 the quality-module split numbers do not follow from the rule | Cumulative versus exclusive `p^n` is a probability-model choice. |
| §6 empty config rendering | `0` versus the empty string is a rendering choice. |
| §6 undefined terms in §6.6 | Needs definitions for modifier power, `power_w` × coefficient, and "names a resource". |
| §6 duplicated selector rows | Needs a decision on which rows and wording to keep. |
| §6 no behaviour is stated for `Unspecified` in §6.6 or §7.7 | Property semantics for that extent need deciding. |
| §7 the exponent rendering names values that cannot exist, and omits the ones that can | Needs a rendering design including non-token glyphs. |
| §7 machine count 1 round-trips to a different value | Choosing between rendering count one and removing the absent/count distinction. |
| §7 the approximate-zero threshold is never defined | Needs a numeric value and unit. |
| §7 an undefined row notation | `288<foundry>` and the two single-line definitions need a notation design. |
| §8 no rule maps a failure to one of the four kinds | Needs an error-kind and message policy. |
| Appendix A "drops" vs "is an error" is never reconciled | Needs the global drop-versus-error decision. |
| Appendix B.4 `a == b` has no semantics | Whether structural equality gets an operator is a design choice. |
| Appendix B.3 (and §2.6) may fail config validation | Needs the recycler slot count from game data. |
| Appendix B.1's exact number depends on a setting | Needs a decision on settings in "exact" examples. |
| Appendix B two T1 assertions are not invariants | Strengthening them would invent claims. |
| Appendix C D20's "unobservable" claim is false | Needs a decision about `f64` grouping observability. |
| Appendix C D44's naming of `:recycling:` is inconsistent with §4.4 | Needs a lexer decision on whether a bare token exists. |
| Appendix C the log omits decisions that are normative elsewhere | Needs new decision entries, which the log's policy forbids inferring. |
| Cross-references §5.1:610 misdescribes its target | Depends on whether `.` reads `resource` or matches terms. |
| Status `[open]` is defined but never used | Needs a status-policy decision. |
