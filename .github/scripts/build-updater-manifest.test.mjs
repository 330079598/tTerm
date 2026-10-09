import assert from "node:assert/strict"
import test from "node:test"

import { buildUpdaterManifest } from "./build-updater-manifest.mjs"

const RELEASE_ASSETS = [
  "tTerm-0.2.2-beta.2-1.x86_64.rpm",
  "tTerm-0.2.2-beta.2-1.x86_64.rpm.sig",
  "tTerm_0.2.2-beta.2_aarch64.dmg",
  "tTerm_0.2.2-beta.2_amd64.AppImage",
  "tTerm_0.2.2-beta.2_amd64.AppImage.sig",
  "tTerm_0.2.2-beta.2_amd64.deb",
  "tTerm_0.2.2-beta.2_amd64.deb.sig",
  "tTerm_0.2.2-beta.2_x64-setup.exe",
  "tTerm_0.2.2-beta.2_x64-setup.exe.sig",
  "tTerm_0.2.2-beta.2_x64.dmg",
  "tTerm_aarch64.app.tar.gz",
  "tTerm_aarch64.app.tar.gz.sig",
  "tTerm_x64.app.tar.gz",
  "tTerm_x64.app.tar.gz.sig",
]

function build(overrides = {}) {
  return buildUpdaterManifest({
    releaseTag: "v0.2.2-beta.2",
    repository: "owner/tTerm",
    releaseNotes: "release notes",
    pubDate: "2026-01-01T00:00:00.000Z",
    assetNames: RELEASE_ASSETS,
    readSignature: (name) => `signature of ${name}`,
    ...overrides,
  })
}

test("maps every updater artifact to the platform keys tauri-action writes", () => {
  const manifest = build()
  const download = "https://github.com/owner/tTerm/releases/download/v0.2.2-beta.2"

  assert.equal(manifest.version, "0.2.2-beta.2")
  assert.equal(manifest.notes, "release notes")
  assert.equal(manifest.pub_date, "2026-01-01T00:00:00.000Z")
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    "darwin-aarch64",
    "darwin-aarch64-app",
    "darwin-x86_64",
    "darwin-x86_64-app",
    "linux-x86_64",
    "linux-x86_64-appimage",
    "linux-x86_64-deb",
    "linux-x86_64-rpm",
    "windows-x86_64",
    "windows-x86_64-nsis",
  ])
  assert.deepEqual(manifest.platforms["darwin-aarch64"], {
    signature: "signature of tTerm_aarch64.app.tar.gz.sig",
    url: `${download}/tTerm_aarch64.app.tar.gz`,
  })
  assert.deepEqual(manifest.platforms["linux-x86_64"], manifest.platforms["linux-x86_64-appimage"])
  assert.equal(manifest.platforms["linux-x86_64-rpm"].url, `${download}/tTerm-0.2.2-beta.2-1.x86_64.rpm`)
  assert.equal(manifest.platforms["windows-x86_64"].url, `${download}/tTerm_0.2.2-beta.2_x64-setup.exe`)
})

test("rejects a release that is missing a platform's artifact", () => {
  assert.throws(
    () => build({ assetNames: RELEASE_ASSETS.filter((name) => !name.includes("setup.exe")) }),
    /Windows NSIS: no artifact matching/
  )
})

test("rejects an artifact uploaded without its signature", () => {
  assert.throws(
    () => build({ assetNames: RELEASE_ASSETS.filter((name) => name !== "tTerm_x64.app.tar.gz.sig") }),
    /macOS x64: tTerm_x64\.app\.tar\.gz has no tTerm_x64\.app\.tar\.gz\.sig/
  )
})

test("rejects an empty signature", () => {
  assert.throws(
    () => build({ readSignature: (name) => (name.endsWith(".deb.sig") ? "\n" : "signature") }),
    /Linux deb: .*\.deb\.sig is empty/
  )
})

test("rejects ambiguous artifacts for one platform", () => {
  assert.throws(
    () => build({ assetNames: [...RELEASE_ASSETS, "t-term_0.2.2-beta.2_amd64.deb"] }),
    /Linux deb: 2 artifacts match/
  )
})

test("rejects an empty release body", () => {
  assert.throws(() => build({ releaseNotes: " \n" }), /Release body is empty/)
})
