# P4-16 appearance: built-in theme choice and light input surfaces

Historical record of the preset-only implementation. The restart requirement below was removed by the [live library change](P4-14-live-theme-library.md).

## What this change adds

- A third settings page, **Appearance**, in the existing settings shell. It
  contains only controls the native client implements:
  - the six built-in themes (`Default`, `Light`, `Ocean`, `Forest`, `Sunset`,
    `Ubuntu`) as selectable preview cards;
  - a **Follow system appearance** switch.
- Persistence through the shared `config.json` fields `theme`,
  `theme_follow_system`, `theme_light` and `theme_dark`.
- Search entries for both controls on the `appearance` page, using the existing
  shared settings index wording.
- A light-mode fix for composed gpui-kit inputs. The settings search field and
  the saved-connections search field now use the same
  `bg-transparent dark:bg-input/30` rule as the production Input; before this
  change they painted `input/30` in every theme.

## Persistence rules

`config/theme_file.rs` starts from the latest file, not from a startup
snapshot. It writes only a theme field that differs from the caller's snapshot,
so:

- unrelated settings and unknown top-level keys survive a theme edit;
- an unchanged custom or catalog theme id is not rejected or replaced;
- a changed theme id must be one of the six built-in themes, because those are
  the only palettes the native client can render;
- the atomic write and read-back match the existing settings edits.

The route never disables **Follow system appearance** on its own. Choosing a
preset while Following is on changes the slot for that theme's brightness and
keeps Following on. Turning Following off is an explicit user action; it freezes
the slot the window would show.

## Why the change applies after a restart

`app_window/theme.rs` resolves the interface and ANSI palettes once, before the
first window opens. Every view receives that `Theme` copy. A live change would
have to re-theme the title bar, tab strip, every session's renderer, saved
connections, tunnels, dialogs and every settings editor, and it would decide
what happens to open drafts. That pass is outside this change.

The Appearance page therefore shows the existing **Restart to apply**
translation under the theme description, and each save stages the preference
for the next launch. The next launch, including the Tauri client, reads the
same fields.

## Tests

- `tterm-core`: theme-file round trip, unknown-key preservation, unchanged
  custom id preservation, invalid changed id refusal.
- `tterm-gpui`: six cards on the Appearance page, staged choice and reload,
  follow-system slot edit without disabling Following, turning Following off,
  search navigation to both controls, unchanged custom id, failed save keeps
  the draft and reports the error.
- `tterm-ui`: the composed field fill is the page background in the light
  theme and tinted in the dark theme.

## Limits

- No custom themes, theme library, theme editor, preset overrides or import in
  the native client.
- No UI style, UI scale, window blur/opacity, tab or pane appearance controls.
- `theme_follow_system` is evaluated at startup. A system appearance change
  while the window is open neither re-themes the window nor changes which slot
  a restart will use until the next launch.
- This is not an Appearance-page acceptance result and has no reference
  screenshot. The first-screen settings comparison still measures the
  Terminal page only.
