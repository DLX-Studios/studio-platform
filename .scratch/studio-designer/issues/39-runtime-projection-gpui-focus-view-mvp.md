# 39 [D]: Runtime Projection v0, GPUI app skeleton, Focus View MVP

**What to build:** Deterministic projection from an immutable Studio Design revision plus Library snapshot into protocol-compatible UiNode trees preserving source identities. GPUI desktop application frame hosting the canvas-first Focus View: open project, click-select nodes on canvas, edit properties via inspector commands, see undo work live. Preview mounts projection output through the retained registry and native renderers.

**Blocked by:** 23, 31, 37

**Status:** ready-for-agent

- [ ] Identical revision produces byte-identical projection output
- [ ] Canvas selection maps bidirectionally with hierarchy identities
- [ ] Inspector edit flows command→revision→projection→visible repaint
- [ ] Undo visibly reverts the last canvas action
- [ ] Source-linked diagnostic appears when projecting an invalid construct
