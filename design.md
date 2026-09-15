---
version: "alpha"
name: "Estilo de Computação de Alta Performance"
description: "Powerful and technical landing page for new server processors. Ideal for landing pages, modern websites. AI-ready template. Palette adapted to the Studio Builder design tokens."
palette: "Studio Builder — dark (see dashboard/src/app.css for the token source of truth)"
colors:
  primary: "#A9B4C2"   # powder-blue — actions, links, highlights (var(--primary))
  secondary: "#1C2321" # carbon-black — app background (var(--background))
  tertiary: "#EEF1EF"  # platinum — foreground, text (var(--foreground))
  neutral: "#5E6572"   # blue-slate — muted structure, borders (var(--secondary))
  surface: "color-mix(in srgb, #1C2321 90%, #A9B4C2 10%)" # card surface (var(--card), resolves ≈ #2A3131)
  accent: "#7D98A1"    # cool-steel — muted accent, secondary text (var(--accent))
typography:
  h1:
    fontFamily: Inter
    fontSize: 2.5rem
    fontWeight: 700
  body-md:
    fontFamily: Inter
    fontSize: 1rem
    fontWeight: 400
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.secondary}"
    padding: 0.75rem
---

## Overview

Powerful and technical landing page for new server processors. Ideal for landing pages, modern websites. AI-ready template. The visual language of high performance computing didn't emerge from design studios. It came from engineering floors, from the need to make invisible power tangible. NVIDIA's green-black palette wasn't arbitrary — it was born from terminal screens and circuit boards, then weaponized into brand identity. AMD went red and angular. Intel stayed blue and clinical. All three arrived at the same conclusion: raw computation needs a visual proxy, something that says 'this thing thinks faster than you.'

The data center aesthetic solidified in the mid-2010s when cloud providers started marketing infrastructure directly. Suddenly, rows of blinking servers became hero images. The visual vocabulary crystallized: deep blacks, electric accent colors, wireframe geometries suggesting parallel pathways, and that specific glow — always a glow — implying energy being converted into intelligence. It's theatrical, sure. But it works because it maps to something real: the sublime scale of coordinated silicon.

What's interesting is how this style borrowed from sci-fi while simultaneously making sci-fi look quaint. The actual inside of an H100 cluster is more visually compelling than most movie sets. Designers figured that out.

- Density: 7/10 — Compact
- Variance: 4/10 — Moderate
- Motion: 4/10 — Subtle

- **Style:** Powerful, Technical, Modern
- **Keywords:** processors, GPUs, data center, high performance, technical, modern, efficient, scalable, reliable, cutting-edge
- **Era:** 2026+ HPC Dominance
- **Light/Dark:** ✗ No / ✓ Full

## Colors

All values map 1:1 to the semantic tokens in `dashboard/src/app.css`. Components must consume the tokens, never the hex literals.

- **Powder Blue** (`#A9B4C2`, `var(--primary)`) — Primary actions, links, focus rings, active indicators. Carries `--primary-foreground: #1C2321` for 8.05:1 contrast on buttons.
- **Carbon Black** (`#1C2321`, `var(--background)`) — Primary background. Deliberately off-black (green-tinted charcoal) per the "no pure black" rule below.
- **Platinum** (`#EEF1EF`, `var(--foreground)`) — Primary text, high-emphasis content. 12.1:1 on carbon-black.
- **Blue Slate** (`#5E6572`, `var(--secondary)`) — Structural elements, table headers, muted fills; mixed at 45% for `var(--border)`.
- **Card Surface** (`var(--card)` — 90% carbon-black / 10% powder-blue mix, ≈ `#2A3131`) — Card and panel backgrounds, with `var(--card-border)`.
- **Cool Steel** (`#7D98A1`, `var(--accent)`) — Secondary text, captions, muted labels (`var(--muted-foreground)`). 6.2:1 on background.
- **Ember** (`#B3605A`, `var(--destructive)`) — Error states, destructive actions. The palette's single auxiliary hue, desaturated to sit within the family's chroma range.
- **Status tokens** — machine activity indicators:
  - **Ready** (`#6FA882`, `var(--status-ready)`) — green; machine idle and reachable
  - **Agent** (`#7D98A1`, `var(--status-agent)`) — AI agent driving the machine
  - **Busy** (`#C9A86A`, `var(--status-busy)`) — compiling / high load
  - **Winding down** (`var(--platinum)`, `var(--status-winding)`) — pulsing; idle timer armed
  - **Off** (`var(--status-off)`) — carbon-black/blue-slate mix; no droplet

## Typography

- **Display / Hero:** Inter (`var(--font-sans)`) — Weight 700, tight tracking, used for headline impact
- **Body:** Inter — Weight 400, 1rem/1.6 line-height, max 72ch per line
- **UI Labels / Captions:** Inter — 0.875rem, weight 500, slight letter-spacing
- **Monospace:** JetBrains Mono (`var(--font-mono)`) — Used for code, metadata, and technical values

Scale:
- Hero: clamp(2.5rem, 5vw, 4rem)
- H1: 2.25rem
- H2: 1.5rem
- Body: 1rem / 1.6
- Small: 0.875rem

## Layout

- **Grid:** CSS Grid primary. Max-width containment: 80rem centered with 1.5rem side padding.
- **Spacing rhythm:** Balanced. Base unit: 0.5rem.
- **Section vertical gaps:** clamp(4rem, 8vw, 8rem).
- **Hero layout:** Split-screen (text left, visual right).
- **Feature sections:** Zig-zag alternating text+image rows. No 3-equal-columns.
- **Mobile collapse:** All multi-column layouts collapse below 48rem. No horizontal overflow.
- **z-index contract:** base (0) / sticky-nav (100) / overlay (200) / modal (300) / toast (500).

## Elevation & Depth

Visualizações de desempenho de chips, diagramas de arquitetura de processadores, brilhos sutis em elementos de alta performance, tipografia técnica e ousada, micro-interações de dados em tempo real, elementos modulares, animações de fluxo de dados e calor.

- **Physics:** Ease-out curves, 200-300ms duration. Smooth and predictable.
- **Entry animations:** Fade + translate-Y (1rem → 0) over 420ms ease-out. Staggered cascades for lists: 80ms between items.
- **Hover states:** Subtle color shift + shadow adjustment over 200ms.
- **Page transitions:** Fade only (200ms).
- **Performance:** Only transform and opacity animated. No layout-triggering properties.
- **Reduced motion:** `prefers-reduced-motion` disables pulses and entry animations.

## Shapes

Base corner radius: `var(--radius-sm)` (0.5rem). Full scale:
- `--radius-sm`: 0.5rem — buttons, inputs, alerts
- `--radius`: 0.75rem — cards, tables, panels
- `--radius-full`: 62.5rem — pills, badges, status dots

## Components

- **Primary Button:** `var(--radius-sm)` shape. `var(--primary)` fill, `var(--primary-foreground)` text. Hover: shift to `var(--primary-hover)` + subtle lift shadow. Active: -0.0625rem translate tactile press. Font weight 600. No outer glows.
- **Secondary / Ghost Button:** Outline variant. `var(--border-width)` border in `var(--border)`. Text in `var(--primary)`. Hover: `color-mix(in srgb, var(--powder-blue) 10%, transparent)` background fill.
- **Cards:** `var(--radius)` corners. `var(--card)` background. Subtle shadow (0 0.125rem 0.75rem rgba(0,0,0,0.06)). `var(--border-width)` stroke in `var(--card-border)`.
- **Inputs:** Label above input. `var(--border-width)` stroke. Focus ring: 0.125rem `var(--ring)` offset 0.125rem. Error text below in `var(--destructive)`. No floating labels.
- **Navigation:** `var(--card)` background. Active item: `var(--primary)` indicator (bottom border in the top bar). Font weight 500 when active.
- **Status Dot:** `var(--radius-full)` circle, 0.625rem. State color from the status tokens. Winding state pulses (1.2s ease-in-out, opacity 1 → 0.25); disabled under reduced motion.
- **Skeletons:** Shimmer animation matching component dimensions, tinted `var(--muted)`. No circular spinners.
- **Empty States:** Icon-based composition (Phosphor, duotone) with descriptive text and action button.

## Do's and Don'ts

- No emojis in UI — use the icon system only (Phosphor icons, `phosphor-svelte`)
- No pure black (#000000) — carbon-black (#1C2321) is the floor
- No oversaturated accent colors (saturation cap: 80%) — all status/auxiliary hues are pre-desaturated
- No 3-column equal-width feature layouts — use zig-zag or asymmetric grid
- No `h-screen` — use `min-h-[100dvh]`
- No AI copywriting clichés: "Elevate", "Seamless", "Unleash", "Next-Gen"
- No broken external image links — use picsum.photos or inline SVG
- No generic lorem ipsum in demos
- No raw px in stylesheets — rem everywhere (see `--border-width`, `--radius*` tokens)
- No hardcoded hex in components — semantic tokens only

- Do Visualizações de desempenho
- Do Diagramas de arquitetura
- Do Brilhos de alta performance
- Do Tipografia técnica
- Do Micro-interações de dados
- Do Animações de fluxo de calor

## Use Case

Landing pages, Modern websites, the Studio Builder dashboard and its future marketing surfaces.
