# Validation

Run studio check examples/stream-countdown, then studio dev examples/stream-countdown. The root page uses a shared layout and imported header/clock; Go live navigates to /live; Back restores root state. Edit an imported component and verify hot reload. Invalid imports must preserve the last valid preview. Focused tests cover parameter/catch-all pages, independent component state and compiled guest navigation. Release packaging must consume the same composed sources; native signed-release acceptance is separate from a source check.
