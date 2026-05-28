## RENAMED Requirements

- FROM: `### Requirement: Lobby Empty State Idle Motion`
- TO: `### Requirement: Lobby Empty State Hero Motion`

## MODIFIED Requirements

### Requirement: Lobby Empty State Idle Motion

The empty-state hero bus emoji SHALL render a continuous cyclic motion: the bus glyph SHALL be horizontally mirrored via `transform: scaleX(-1)` so the bus faces left, and SHALL translate along the horizontal axis from approximately -50 pixels to approximately +50 pixels (a total traversal of approximately 100 pixels), with a vertical bumpy displacement of approximately -3 pixels at the mid-traversal keyframes, and a rotation oscillating within approximately ±2 degrees. The motion SHALL loop with a duration of approximately 2.5 seconds and SHALL follow a dwell-and-return phasing in which the bus advances to the far endpoint, dwells briefly at that endpoint, and then returns to the starting endpoint before repeating. The motion SHALL be implemented as pure CSS keyframes; no JavaScript animation library SHALL be introduced for this effect.

When the user agent advertises `prefers-reduced-motion: reduce`, the cyclic motion SHALL be suppressed and the bus emoji SHALL remain completely static; no transform animation SHALL be applied.

The cyclic motion SHALL be confined to the empty-state hero only; the topbar bus wordmark glyph SHALL remain static, and no other Lobby element SHALL animate as a consequence of this requirement. The LoadingOverlay bus animation defined elsewhere SHALL NOT be affected by this requirement.

#### Scenario: Hero bus animates with mirrored cyclic motion by default

- **WHEN** a user opens the Lobby in empty state on a system that does not advertise `prefers-reduced-motion: reduce`
- **THEN** the hero bus emoji SHALL render horizontally mirrored (facing left) AND SHALL visibly translate horizontally across an approximately 100-pixel span (-50px to +50px) within a continuous loop of approximately 2.5 seconds, with a visible rotation oscillation within approximately ±2 degrees and a bumpy vertical displacement of approximately -3 pixels at the mid-traversal keyframes, AND SHALL exhibit a dwell-and-return phasing in which the bus pauses briefly at the far endpoint before returning to the starting endpoint

##### Example: motion keyframe progression

| Keyframe | Horizontal | Vertical | Rotation | Notes |
| -------- | ---------- | -------- | -------- | ----- |
| 0% / 100% | -50px | 0 | -2deg | start / loop boundary |
| 20% | -25px | -3px | 0deg | mid forward bump |
| 45% | +15px | 0 | +2deg | crossing center |
| 65% | +50px | -3px | 0deg | far endpoint reached |
| 75% | +50px | 0 | -1deg | dwell at endpoint |
| (75% → 100%) | +50px → -50px | 0 | -1deg → -2deg | return arc |

#### Scenario: Reduced-motion preference disables cyclic motion

- **WHEN** the user agent advertises `prefers-reduced-motion: reduce`
- **THEN** the hero bus emoji SHALL render with no transform animation applied; computed `animation-name` SHALL be `none` (or equivalent)

#### Scenario: Cyclic motion is scoped to empty-state hero

- **WHEN** the cyclic motion is active in the empty state
- **THEN** the topbar bus wordmark glyph SHALL render statically AND the LoadingOverlay bus animation SHALL retain its own single-direction non-mirrored keyframes AND every other Lobby element SHALL render without any motion attributable to this requirement
