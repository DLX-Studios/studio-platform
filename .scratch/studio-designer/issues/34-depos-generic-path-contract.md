# 34 [R]: Remove POS-specific rendering from the generic path

**What to build:** All point-of-sale identifiers and branches (`order-pane`, `catalog-pane`, `add-*` conventions) leave the generic render path; certification forbids fallback rendering for certified kinds. The POS example renders identically through purely generic machinery.

**Blocked by:** 31, 32, 33

**Status:** ready-for-agent

- [ ] Example renders pixel-equivalent through the generic path
- [ ] Audit shows zero POS-specific identifiers or branches in generic code
- [ ] Release certification fails if any approved kind lacks semantic rendering
- [ ] Fallback surfaces exist only behind an explicit development diagnostic
