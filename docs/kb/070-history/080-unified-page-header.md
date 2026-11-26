---
summary: >-
  `2026-11-26`: Home, Add session and the session page share one header,
  styled in `web/src/header.css`, with the same height (117.2 px), width and
  position on every page at every width from 320 to 1920 px, measured in
  headless Brave; `make dev` now refuses to start when `:3000` or `:5173` is
  taken.
---

# Unified page header (`2026-11-26`)

The maintainer staged a look unification (one generic `header` rule and one
48×78 px brand mark on every page) and asked:

> i made some changes to unify look (staged) can u please make sure header has
> same size on all subpages for same screen size

## What differed

Measured in headless Brave before the change: on desktop the session header
was 117.2 px tall and the others 107.2 px, because the session page's logo
sits inside the 88 px `.lobby-link` pill; from 900 px down the Home and Add
session headers wrapped their status chip or back link onto a second row; at
760 px and below every header stacked into a column, and the session
header's `flex: 1 1 280px` became 280 px of height there (431 px tall at
390 px). The session page also sat 7 px off, or 15 px narrower below 1280 px,
because only it scrolled and so lost the scrollbar's width.

## What changed

- `web/src/header.css` holds every header rule, moved out of `navbar.css` and
  `lobby.css`. The header's `min-height` is one row as tall as the logo pill
  plus padding, computed from the same custom properties the pill and brand
  mark use, and every header's content stays on that one row at all widths.
  Text shrinks or truncates instead of wrapping onto more rows: Home and Add
  session clamp the description to two lines; at 760 px and below the
  description hides and the title may wrap to three lines; long session
  names end in an ellipsis.
- At 760 px and below, `.wide-only` text is visually hidden but still read by
  screen readers: "Back to " on Add session, "Lobby" in the session pill,
  and the counter words "Orange" and "Red". At 600 px and below,
  `.session-nav-main` (title, counters, status chip; new in
  `SessionHeader.tsx`) becomes a grid with the chip under the title, and the
  chip's label truncates. `display: contents` would have avoided the wrapper
  but is outside the project's Biome Baseline, as is `scrollbar-gutter`.
- `html { overflow-y: scroll }` keeps the scrollbar on every page, so short
  and long pages share one width and position.
- Cleanups in the staged files: the unused `LOGO_SIZE`, trailing spaces in
  `className="brand-mark "`, the dead `lobby-header` class, and the home
  render test updated to the new description.
- `make dev` checks with `curl` that `:3000` and `:5173` are free before
  building, and Vite uses `strictPort`. A second `make dev` had started with
  its server failing to bind `:3000` ("server exited") and Vite quietly
  moving to `:5174` while the banner said `:5173`.

## Verification

- Headless Brave at 16 widths (1920 to 320 px) on Home, Add session, a
  session and a session with a 60-character name: equal height, width and
  left edge on all four at every width, and no header element outside the
  header box. Screenshots checked at 1600, 900 and 390 px.
- `make gate` and `make sonar` before committing.
