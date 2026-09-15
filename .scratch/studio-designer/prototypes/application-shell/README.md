# Studio Designer application-shell review prototype

Retained raw HTML/CSS/JavaScript review artifact for validating the full Studio Designer desktop journey and checking production fidelity later. It remains prototype code rather than production architecture.

Run from the repository root:

```bash
python3 -m http.server 4174 --bind 127.0.0.1 --directory .scratch/studio-designer/prototypes/application-shell
```

Open `http://127.0.0.1:4174/?variant=A&screen=welcome`.

- Welcome A — Monolith: the default split welcome direction.
- Welcome B — Topology: the alternate dotted spatial welcome direction.
- Desktop shell — Monolith: one desktop-native title bar across welcome, identity, project chooser, settings, recovery, and editor screens.

Use the **Prototype journey** control to jump between screens. On the Project Dashboard, switch among **Grid**, **Index**, and **Activity**, then open any project to enter the integrated canvas editor. The editor defaults to **Focus** and can switch to **Workbench** from the native title bar while retaining the same fixture selection, profile, mode, and agent state. The floating bottom control and Left/Right arrow keys compare A and B only while the welcome screen is active.

In Focus View, the compact tool rail is vertically centered. Portrait and Landscape controls beside the responsive-profile chooser rotate the preview by swapping its effective dimensions while preserving editor state. The Studio Agent composer presents attachment, canvas-context, model-and-effort, voice, and send controls in one toolbar. The attachment control opens a searchable source palette for uploads, Studio Library apps/pages/components/media, URLs, Google Drive, Figma, Dropbox, and OneDrive; selected references appear as removable context cards above the prompt input. The adjacent cursor control enters a one-shot canvas picker: eligible canvas nodes highlight on hover, clicking one adds its stable semantic reference to the same context strip, and Escape cancels. File picking and drag/drop are local prototype interactions, while external providers remain fixture states without network access. The model control opens its own provider palette with model metadata, selection state, and Low/High/Max reasoning controls. The conversation can remain a floating canvas window or become a full-height Left or Right pane without changing its selected context or model. Workbench exposes the same shared model palette from Agent Activity. Preview and Build are available from the editor title bar's **•••** menu.

Authenticated back navigation follows the intended journey: Canvas Editor → Project Dashboard → Account Chooser. Settings and Recovery return to the authenticated screen that opened them. Every screen retains a draggable title region and window controls; minimize, maximize, and close remain visual prototype affordances because only the native desktop host can execute those operations.

This is deliberately non-production code, but it is retained for later review and fidelity comparison. It has no persistence, authentication, or production backend.

The styling is a plain-CSS translation of Studio Canvas's protected mono design profile and implemented welcome, account, onboarding, project-dashboard, and topology surfaces. It intentionally uses square geometry, monochrome semantic tokens, mono metadata, grid-backed setup pages, and 24px dotted topology fields without importing Tailwind or Svelte.
