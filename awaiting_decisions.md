
## Post-implementation questions

1. **Inserter capacity research effects.** The setting `inserter_capacity` is
   currently treated as a flat bonus per level. Should it instead sum the
   actual `inserter-stack-size-bonus` / `bulk-inserter-capacity-bonus` research
   effects from game data?
2. **Mixed beacon types.** Beacon transmission uses the first beacon type's
   `profile` for the total beacon count. Is a mixed `beacon-a + beacon-b`
   configuration allowed to share one profile, or should each group use its own
   profile?
3. **Process unit selectors.** `.:<item>:` returns the net row without a
   rate/total time unit. Should it carry `second^-1` for `Machines`/unsolved
   processes and no time unit for `MachineSeconds`?
