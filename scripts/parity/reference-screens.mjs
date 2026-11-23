/**
 * The two screen collections in `parity/screens.json`.
 *
 * `screens` is the contract shared with the GPUI comparison. `referenceScreens`
 * holds React-only states that are captured before their GPUI view exists; the
 * shared reference manifest must not list them, or the comparison would
 * invalidate its cache on every run.
 */

export function pairedScreens(manifest) {
  return manifest.screens
}

export function referenceScreens(manifest) {
  return manifest.referenceScreens ?? []
}

export function screenCollections(manifest) {
  const paired = pairedScreens(manifest)
  const reference = referenceScreens(manifest)
  const ids = new Set()
  for (const screen of [...paired, ...reference]) {
    if (!screen.id) {
      throw new Error("parity/screens.json has a screen without an id")
    }
    if (ids.has(screen.id)) {
      throw new Error(`parity/screens.json lists "${screen.id}" twice`)
    }
    ids.add(screen.id)
  }
  return { paired, reference }
}

/**
 * Resolves the screens for one capture. With no `--screen` arguments the
 * default capture covers only the paired screens, so the shared reference
 * manifest keeps its meaning and its cache.
 */
export function selectScreens(manifest, requestedIds) {
  const { paired, reference } = screenCollections(manifest)
  if (requestedIds.length === 0) {
    return { selected: paired, paired, reference: [] }
  }

  const byId = new Map([...paired, ...reference].map((screen) => [screen.id, screen]))
  const selected = []
  const seen = new Set()
  for (const id of requestedIds) {
    if (seen.has(id)) continue
    const screen = byId.get(id)
    if (!screen) {
      throw new Error(`No parity screen "${id}"; manifest ids: ${[...byId.keys()].join(", ")}`)
    }
    seen.add(id)
    selected.push(screen)
  }

  return {
    selected,
    paired: selected.filter((screen) => paired.includes(screen)),
    reference: selected.filter((screen) => reference.includes(screen)),
  }
}

/** The shared reference manifest, covering the paired screens just captured. */
export function referenceManifest(
  screens,
  { version, commit, theme, uiStyle, uiScalePercent, viewport }
) {
  return {
    version,
    commit,
    theme,
    uiStyle,
    uiScalePercent,
    viewport,
    screens: screens.map((screen) => ({
      id: screen.id,
      image: `${screen.id}.png`,
      layout: `${screen.id}.layout.json`,
      ...(screen.locale ? { locale: screen.locale } : {}),
    })),
  }
}

/** Describes the reference-only captures without touching the shared manifest. */
export function referenceOnlyManifest(
  screens,
  { version, commit, theme, uiStyle, uiScalePercent, viewport }
) {
  return {
    version,
    commit,
    theme,
    uiStyle,
    uiScalePercent,
    viewport,
    screens: screens.map((screen) => ({
      id: screen.id,
      image: `${screen.id}.png`,
      layout: `${screen.id}.layout.json`,
      ...(screen.locale ? { locale: screen.locale } : {}),
    })),
  }
}
