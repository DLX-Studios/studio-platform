# Studio Designer editor-shell review prototype

Retained UI review artifact recording the original native Studio Designer workspace directions.

Run from the repository root:

```bash
python3 -m http.server 4173 --directory .scratch/studio-designer/prototypes/editor-shell
```

Open `http://127.0.0.1:4173/?variant=A` and use the floating switcher or Left/Right arrow keys.

- A — Workbench: persistent hierarchy, stage, inspector, and compact activity.
- B — Focus Studio: canvas-first authoring with contextual floating tools.
- C — Flow Deck: journey-oriented navigation with an integrated authoring deck.

This code remains intentionally non-production, but is retained for later comparison against the implemented editor. It does not represent production architecture.
