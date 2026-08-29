# 42 [D]: Responsive profiles and breakpoint overrides

**What to build:** Base-plus-breakpoint responsive value editing; device-profile preview switching across the full matrix (phone through 4K) honoring orientation, safe areas, pixel ratio, and input metadata; side-by-side comparison surfacing unintended differences.

**Blocked by:** 38, 41

**Status:** done

- [x] Override set at a breakpoint renders only there and is visible in inspector provenance
- [x] Compare view flags unintended differences between chosen profiles
- [x] Profile switching preserves selection and canvas state

## Closure audit at `3a05109`

- [x] Breakpoint override resolves only for its matching profile and exposes provenance — `crates/studio-design/tests/responsive_profiles.rs:102-160`, inspector provenance query at `:261-272`, and resolution/inspection implementation in `crates/studio-design/src/responsive.rs:250-429`.
- [x] Compare view reports differences with authored/unintended classification — `crates/studio-design/tests/responsive_profiles.rs:102-160` and `crates/studio-design/src/responsive.rs:584-655`.
- [x] Profile switching preserves selection/canvas state and now updates Focus projection — context preservation: `crates/studio-design/tests/responsive_profiles.rs:162-274`; device-profile→variant projection mapping: `crates/studio-designer/src/focus_view.rs:162-184`; regression journey: `crates/studio-designer/tests/focus_view.rs:347-369`.

Verdict: closed. The Focus profile mapping gap found during audit is fixed and covered by regression evidence; all responsive suites pass.
