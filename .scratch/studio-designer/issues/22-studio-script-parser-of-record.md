# 22 [R]: Studio Script parser of record, canonical printer, check/fmt

**What to build:** The rsvelte-flavored grammar parses `.studio` sources into a stable-ID semantic model; a deterministic canonical printer serializes it back; `studio check` validates and reports structured diagnostics; `studio fmt` enforces canonical formatting. Comments persist as trivia anchored to their following node.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Round-trip invariant holds: parsing printed output reproduces the identical model
- [ ] Printing is deterministic: identical models yield byte-identical output across runs and machines
- [ ] Missing, duplicate, or non-canonical node identities are rejected with line-linked diagnostics
- [ ] Comments survive parse/print cycles anchored to their nodes
- [ ] Check exits nonzero with stable diagnostic codes on the invalid-fixture corpus
- [ ] Fixture corpus covers valid and invalid documents including hostile input
