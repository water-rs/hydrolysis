# AGENTS.md

## Unsupported components panic — never stub

Hydrolysis never draws a stand-in for a component it cannot realize. A view
that reaches the backend without a realization — e.g. `Native<MapConfig>`
because no `Hook<MapConfig>` (such as `waterui_map_gpu::install`) was
installed — is a programmer error and must panic at the earliest point it is
seen (measure or node build) with a message naming the missing piece and how to
install it. See `unsupported_system_icon` and `unsupported_map` in
`src/renderer/native_measure.rs` for the pattern.

Do not add gradient/color/mock substitutes, silent no-ops, or fallback
renderings for unsupported primitives — a stand-in that merely *looks* like the
component hides the missing realization from the application author.
