import assert from "node:assert/strict"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"

import { synchronizeManifestNotes } from "./sync-updater-manifest-notes.mjs"

function createManifest(directory, name, version, notes = "old notes") {
  const manifestPath = join(directory, name)
  writeFileSync(manifestPath, `${JSON.stringify({ version, notes, platforms: {} }, null, 2)}\n`)
  return manifestPath
}

test("updates every matching manifest after validating the full set", () => {
  const directory = mkdtempSync(join(tmpdir(), "tterm-manifest-notes-"))
  try {
    const primary = createManifest(directory, "primary.json", "0.2.2-beta.2")
    const legacy = createManifest(directory, "legacy.json", "0.2.2-beta.2")

    const result = synchronizeManifestNotes({
      manifestPaths: [primary, legacy],
      releaseTag: "v0.2.2-beta.2",
      releaseNotes: "new notes",
      strictVersionMatch: true,
    })

    assert.deepEqual(result, { changed: true, skipped: false, reason: "Updated 2 manifest(s)" })
    assert.equal(JSON.parse(readFileSync(primary, "utf8")).notes, "new notes")
    assert.equal(JSON.parse(readFileSync(legacy, "utf8")).notes, "new notes")
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test("skips an automatic sync without changing any manifest when a version is stale", () => {
  const directory = mkdtempSync(join(tmpdir(), "tterm-manifest-notes-"))
  try {
    const primary = createManifest(directory, "primary.json", "0.2.2-beta.2")
    const legacy = createManifest(directory, "legacy.json", "0.2.2-beta.3")

    const result = synchronizeManifestNotes({
      manifestPaths: [primary, legacy],
      releaseTag: "v0.2.2-beta.2",
      releaseNotes: "new notes",
      strictVersionMatch: false,
    })

    assert.equal(result.changed, false)
    assert.equal(result.skipped, true)
    assert.match(result.reason, /legacy\.json is 0\.2\.2-beta\.3/)
    assert.equal(JSON.parse(readFileSync(primary, "utf8")).notes, "old notes")
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test("rejects a manual or release-time sync when a manifest has the wrong version", () => {
  const directory = mkdtempSync(join(tmpdir(), "tterm-manifest-notes-"))
  try {
    const manifest = createManifest(directory, "latest.json", "0.2.2-beta.3")

    assert.throws(
      () =>
        synchronizeManifestNotes({
          manifestPaths: [manifest],
          releaseTag: "v0.2.2-beta.2",
          releaseNotes: "new notes",
          strictVersionMatch: true,
        }),
      /Manifest version mismatch/
    )
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test("rejects an empty release body", () => {
  const directory = mkdtempSync(join(tmpdir(), "tterm-manifest-notes-"))
  try {
    const manifest = createManifest(directory, "latest.json", "0.2.2-beta.2")

    assert.throws(
      () =>
        synchronizeManifestNotes({
          manifestPaths: [manifest],
          releaseTag: "v0.2.2-beta.2",
          releaseNotes: " \n",
          strictVersionMatch: true,
        }),
      /Release body is empty/
    )
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})
