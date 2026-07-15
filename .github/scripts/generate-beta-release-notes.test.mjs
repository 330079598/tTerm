import assert from "node:assert/strict"
import test from "node:test"

import { buildReleaseNotes, classifyCommit } from "./generate-beta-release-notes.mjs"

test("classifies repository Gitmoji subjects", () => {
  assert.deepEqual(classifyCommit(":sparkles: add jump host support"), {
    category: "Features",
    description: "add jump host support",
  })
  assert.deepEqual(classifyCommit(":bug: preserve terminal values"), {
    category: "Fixes",
    description: "preserve terminal values",
  })
  assert.deepEqual(classifyCommit(":art: improve tab scrolling"), {
    category: "UI and usability",
    description: "improve tab scrolling",
  })
})

test("supports Conventional Commits and excludes internal changes", () => {
  assert.deepEqual(classifyCommit("feat(sftp): add path editor"), {
    category: "Features",
    description: "add path editor",
  })
  assert.equal(classifyCommit("chore: update tooling"), null)
  assert.equal(classifyCommit(":green_heart: update CI"), null)
  assert.equal(classifyCommit(":recycle: extract helper"), null)
})

test("builds grouped notes with commit and comparison links", () => {
  const notes = buildReleaseNotes({
    commits: [
      { sha: "a".repeat(40), subject: ":bug: fix [path] handling" },
      { sha: "b".repeat(40), subject: ":sparkles: add path editor" },
      { sha: "c".repeat(40), subject: "test: add coverage" },
    ],
    previousTag: "v0.2.2-beta.1",
    betaTag: "v0.2.2-beta.2",
    repository: "330079598/tTerm",
  })

  assert.match(notes, /### Features/)
  assert.match(notes, /### Fixes/)
  assert.ok(notes.includes("fix \\[path\\] handling"))
  assert.match(notes, /\/commit\/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/)
  assert.doesNotMatch(notes, /add coverage/)
  assert.match(notes, /compare\/v0\.2\.2-beta\.1\.\.\.v0\.2\.2-beta\.2/)
})

test("explains when all commits are filtered out", () => {
  const notes = buildReleaseNotes({
    commits: [{ sha: "d".repeat(40), subject: "ci: update runner" }],
    previousTag: "v0.2.2-beta.2",
    betaTag: "v0.2.2-beta.3",
    repository: "330079598/tTerm",
  })

  assert.match(notes, /No user-facing changes were detected/)
})
