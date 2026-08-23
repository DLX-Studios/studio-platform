# ADR-0002: Retained UI and Host-Driven Animation

- Status: Accepted
- Date: 2026-08-03

## Decision

Plugins emit an initial retained UI tree and later send atomic structural or property patches. The host owns layout, rendering, focus, accessibility, overlays, and animation scheduling.

Studio does not expose a guest frame loop or immediate-mode canvas in the initial protocol.

## Rationale

This keeps work proportional to state changes, allows native widgets to preserve interaction state, and prevents an idle plugin from consuming CPU simply to redraw an unchanged interface.

