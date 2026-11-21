import assert from "node:assert/strict"
import { test } from "node:test"

import {
  referenceManifest,
  referenceOnlyManifest,
  screenCollections,
  selectScreens,
} from "./reference-screens.mjs"

const manifest = {
  screens: [{ id: "example.button-input.default" }, { id: "ui.button.variants" }],
  referenceScreens: [{ id: "workspace.titlebar.default" }],
}

const details = {
  version: 2,
  commit: "abc123",
  theme: "dark",
  uiStyle: "classic",
  uiScalePercent: 100,
  viewport: { width: 800, height: 600, deviceScaleFactor: 2 },
}

test("a default capture selects paired screens and excludes reference-only ones", () => {
  const selection = selectScreens(manifest, [])
  assert.deepEqual(
    selection.selected.map((screen) => screen.id),
    ["example.button-input.default", "ui.button.variants"]
  )
  assert.equal(selection.reference.length, 0)
})

test("the shared reference manifest lists only paired screens", () => {
  const output = referenceManifest(manifest.screens, details)
  assert.deepEqual(
    output.screens.map((screen) => screen.id),
    ["example.button-input.default", "ui.button.variants"]
  )
})

test("an explicit paired capture lists only the captured subset", () => {
  const selection = selectScreens(manifest, ["ui.button.variants"])
  const output = referenceManifest(selection.paired, details)
  assert.deepEqual(output.screens.map((screen) => screen.id), ["ui.button.variants"])
})

test("an explicit screen can select a reference-only screen", () => {
  const selection = selectScreens(manifest, ["workspace.titlebar.default"])
  assert.deepEqual(selection.selected.map((screen) => screen.id), [
    "workspace.titlebar.default",
  ])
  assert.equal(selection.paired.length, 0)
  assert.equal(selection.reference.length, 1)

  const output = referenceOnlyManifest(selection.reference, details)
  assert.deepEqual(output.screens.map((screen) => screen.id), ["workspace.titlebar.default"])
})

test("an explicit paired screen stays out of the reference-only manifest", () => {
  const selection = selectScreens(manifest, ["ui.button.variants"])
  assert.equal(selection.reference.length, 0)
  assert.deepEqual(selection.paired.map((screen) => screen.id), ["ui.button.variants"])
})

test("unknown screen ids are rejected", () => {
  assert.throws(() => selectScreens(manifest, ["missing.screen"]), /No parity screen "missing.screen"/)
})

test("a screen may not appear in both collections", () => {
  const duplicated = {
    screens: [{ id: "same.screen" }],
    referenceScreens: [{ id: "same.screen" }],
  }
  assert.throws(() => screenCollections(duplicated), /lists "same.screen" twice/)
})
