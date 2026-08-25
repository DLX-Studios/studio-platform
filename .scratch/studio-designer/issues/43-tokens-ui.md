# 43 [D]: Design tokens UI

**What to build:** Create, edit, apply, and override color/typography/spacing/radius/border/shadow/motion tokens. Inspector distinguishes shared intent from local values. Renames propagate while identities stay stable.

**Blocked by:** 38, 39

**Status:** ready-for-agent

- [ ] Apply→override→clear flow works per property
- [ ] Rename propagates to all consumers without identity churn
- [ ] Deleting a referenced token requires confirmation listing usages
- [ ] Token use shown for every inspected value
