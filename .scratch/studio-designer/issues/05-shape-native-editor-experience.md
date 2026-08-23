# Shape the native editor experience

Type: prototype
Status: resolved
Blocked by: 03, 04

## Question

How should the native canvas, hierarchy, component insertion, responsive layout editing, explicit overlay placement, inspector, Studio Library, interaction authoring, live agent activity, history, preview, and build surfaces behave as one coherent desktop workflow?

## Answer

Studio Designer has two fully supported workspace views over one editor session:

- **Focus View** is the default. It follows prototype B: the canvas dominates, tools float near the work, the selected node opens a contextual inspector, and live agent activity plus its undo group remains visible in a bottom dock.
- **Workbench View** follows prototype A. It keeps screens, hierarchy, Studio Library, canvas controls, inspector, diagnostics, interactions, agent activity, and history visible for deep authoring and debugging.

The views are presentations of the same editor modules, not separate editors or document modes. Switching views preserves the active project and screen, selected node, device profile, canvas transform, current tool, live agent operation, undo/redo history, diagnostics, and unsaved changes. Each view may remember its own panel sizes and collapsed state without duplicating domain state.

No authoring capability is exclusive to Workbench View. Focus View exposes its less-frequent surfaces through collapsible edge panels and the command bar:

- screens, hierarchy, and Studio Library open from the left edge;
- node properties, content bindings, and interaction authoring open contextually from the right edge;
- agent activity, operation history, diagnostics, and interaction traces open from the bottom dock;
- device profiles, zoom, preview, and build remain available from the canvas or top controls;
- Design, Prototype, and Data are workspace destinations available in both views.

Focus View and Workbench View share one command registry, selection model, panel registry, and editor-session interface. A visible workspace-view control and keyboard command switch between them. Focus View optimizes uninterrupted composition; Workbench View optimizes simultaneous visibility and inspection.

The validated throwaway prototype is at [`../prototypes/editor-shell/index.html`](../prototypes/editor-shell/index.html), with variants B and A representing the two accepted views. Variant C is not a primary workspace, though its journey and build-readiness ideas may appear as panels inside both accepted views.
