# Retained Studio Designer review prototypes

These prototypes are intentionally retained as design-review and fidelity-check sources. They are not production architecture and should not be deleted during implementation cleanup until the Studio Designer application has been compared against them.

## Complete retained inventory

Nothing under this directory is excluded from retention.

### Application shell

`application-shell/index.html` retains every switchable application page and its in-memory interaction states:

- Product welcome — A / Monolith and B / Topology.
- Account chooser.
- Account-type chooser.
- Create local identity.
- Local login.
- Cloud login.
- Four-step cloud onboarding.
- Project dashboard — Grid, Index, and Activity modes, including recent items and search.
- Canvas editor — Focus and Workbench views; Design, Prototype, and Data destinations; all responsive profiles and both orientations.
- Settings and providers.
- Conflict and recovery.
- Agent chat — floating, left-docked, and right-docked; model/effort palette; asset providers; uploaded and URL context; canvas-context picking; voice state; inline semantic references; and removable context cards.
- Editor overlays and states — selection, inspector tabs, history/activity, preview/build menu, zoom, save status, and responsive validation fixtures.

`application-shell/qa/screenshots/` retains all 13 visual checkpoints, including compact-window captures and earlier A, B, and C explorations. These images remain useful even where the live prototype later converged on a different direction.

### Earlier editor comparison

`editor-shell/index.html` retains all three original editor directions:

- A — Persistent Workbench.
- B — Canvas-first Focus Studio.
- C — Flow-oriented Deck.

Its selection, profile, design mode, live-agent, history, synchronization, and zoom fixture states are also part of the retained artifact.

## Fidelity-review use

Use these pages to compare production work for:

- Canvas-derived monochrome design language, grid and dotted backgrounds, typography, spacing, and desktop-native title chrome.
- Navigation continuity from identity selection through projects and editor authoring.
- Focus and Workbench information hierarchy.
- Responsive-profile, orientation, inspector, Studio Library, history, and agent interaction placement.
- Agent model selection, asset attachment, canvas-context picking, voice control, and float/left/right chat layouts.

The fixtures are local-only and intentionally contain no production authentication, provider integration, persistence, or runtime implementation.

## Run

From the repository root:

```bash
python3 -m http.server 4174 --bind 127.0.0.1 --directory .scratch/studio-designer/prototypes/application-shell
```

Then open `http://127.0.0.1:4174/?variant=A&screen=welcome`.

For the earlier editor comparison:

```bash
python3 -m http.server 4173 --bind 127.0.0.1 --directory .scratch/studio-designer/prototypes/editor-shell
```

Then open `http://127.0.0.1:4173/?variant=A`.

## Retention rule

Keep every file, embedded page, variant, fixture state, and QA screenshot beneath this directory until the corresponding production screens have passed a deliberate fidelity review. Do not discard an earlier direction merely because a later direction became the default. If these artifacts are eventually removed from the implementation branch, preserve their final state in repository history with a link from the implementation record.
