# Studio routing contract v1

Pages: routes/+page.studio -> /; routes/live/+page.studio -> /live; routes/streams/[id]/+page.studio -> /streams/:id; routes/[...rest]/+page.studio -> /*. routes/index.studio and routes/live.studio remain shorthand. Files/directories beginning with _ are private; +layout.studio is never a page.

Layout: +layout.studio wraps each descendant page through its default outlet. Component imports: import Clock from '../components/Clock.studio'; only local default .studio imports are supported. Props come from $props(); imported instance IDs and reactive slots are scoped to avoid collisions. Imports resolving outside the project, cycles and missing imports are errors.

Navigation handlers emit('navigate', { operation: 'push', route: '/live' }); supported operations are the existing push/replace/pop/pop_to/reset variants. The compiler emits the existing protocol Navigate envelope; the host validates declarations/history and sends the existing Navigation result before the guest mounts the selected page. Unrequested mounts fail. Generated routing metadata is a signed asset with schemaVersion 1, at most 64 pages and 1 MiB total; source paths are relative and IDs/patterns unique.

No browser timers, server modules or arbitrary native imports are admitted by this contract.

## Bounded host timers

`<Timer id="stream-timer" interval_ms={1000} until={remaining === 0} completion_delay_ms={700} ontick={tick} oncomplete={goLive} />` is a compiler-only component. It lowers to invisible native nodes and existing pressed handlers. No new native kind, guest import, protocol envelope or per-frame guest callback is added. Timer metadata is compiler-generated under assets/studio/timers.json (schemaVersion 1, <=1 MiB, <=64 pages, <=16 timers/page). IDs are unique, scoped and bounded; intervals are integer 1000–60000 ms and completion delay 0–60000 ms. The host validates signed metadata and node ownership before dispatch. A status node projects 'running' or 'complete'; the host stops ticks and schedules one completion after the delay. Leaving a page cancels its pending schedule; returning resumes retained page state with a fresh interval. Catch-up delivers at most 32 events per host poll and no guest call while nothing is due. Manual-clock tests cover drift, bounds, stop and completion. No new authority to network, files or secrets is granted.


## Native landing-page presentation

Box.background_asset accepts only an assets/ path admitted by the project manifest. The host paints it behind the Box content, covering its bounds without changing layout or input targets. Text.font_size is a finite number 8–256 logical pixels scaled by preview zoom; Text.color is exactly #RRGGBB. No arbitrary CSS, font files, native classes or guest drawing hooks are admitted. Static title artwork may contain outlined lettering with source/license attribution; it contains no working font. Existing transition.duration_ms/curve and opacity properties interpolate retained native values, including interruption and zero-duration reduced motion.
