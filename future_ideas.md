# Future ideas

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
