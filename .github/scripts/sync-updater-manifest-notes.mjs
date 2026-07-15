import {
  appendFileSync,
  existsSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs"
import { execFileSync } from "node:child_process"
import { pathToFileURL } from "node:url"

function expectedVersion(releaseTag) {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(releaseTag)) {
    throw new Error(`Invalid release tag: ${releaseTag}`)
  }
  return releaseTag.slice(1)
}

function writeGithubOutput(name, value) {
  const outputPath = process.env.GITHUB_OUTPUT
  if (outputPath) appendFileSync(outputPath, `${name}=${value}\n`, "utf8")
}

function writeSummary(lines) {
  const summaryPath = process.env.GITHUB_STEP_SUMMARY
  if (summaryPath) appendFileSync(summaryPath, `${lines.join("\n")}\n`, "utf8")
}

function summaryLines({ releaseTag, status, reason }) {
  const lines = [
    "## Updater manifest release notes",
    "",
    `- Release: \`${releaseTag}\``,
    `- Status: ${status}`,
  ]
  if (reason) lines.push(`- Reason: ${reason}`)
  return lines
}

export function synchronizeManifestNotes({
  manifestPaths,
  releaseTag,
  releaseNotes,
  strictVersionMatch,
}) {
  if (!releaseNotes.trim()) throw new Error(`Release body is empty for ${releaseTag}`)
  if (manifestPaths.length === 0) throw new Error("At least one manifest path is required")

  const version = expectedVersion(releaseTag)
  const manifests = manifestPaths.map((manifestPath) => {
    if (!existsSync(manifestPath)) throw new Error(`Manifest not found: ${manifestPath}`)

    let manifest
    try {
      manifest = JSON.parse(readFileSync(manifestPath, "utf8"))
    } catch (error) {
      throw new Error(`Failed to parse manifest ${manifestPath}: ${error.message}`)
    }

    return { manifestPath, manifest }
  })

  const mismatches = manifests.filter(({ manifest }) => manifest.version !== version)
  if (mismatches.length > 0) {
    const reason = mismatches
      .map(
        ({ manifestPath, manifest }) =>
          `${manifestPath} is ${manifest.version ?? "missing"}, expected ${version}`
      )
      .join("; ")

    if (strictVersionMatch) throw new Error(`Manifest version mismatch: ${reason}`)

    return { changed: false, skipped: true, reason }
  }

  const updates = manifests
    .filter(({ manifest }) => manifest.notes !== releaseNotes)
    .map(({ manifestPath, manifest }) => ({
      manifestPath,
      content: `${JSON.stringify({ ...manifest, notes: releaseNotes }, null, 2)}\n`,
    }))

  if (updates.length === 0) {
    return { changed: false, skipped: false, reason: "Release notes are already up to date" }
  }

  const temporaryFiles = updates.map(({ manifestPath, content }) => {
    const temporaryPath = `${manifestPath}.notes-${process.pid}.tmp`
    writeFileSync(temporaryPath, content, "utf8")
    return { manifestPath, temporaryPath }
  })

  try {
    for (const { manifestPath, temporaryPath } of temporaryFiles) {
      renameSync(temporaryPath, manifestPath)
    }
  } finally {
    for (const { temporaryPath } of temporaryFiles) {
      if (existsSync(temporaryPath)) unlinkSync(temporaryPath)
    }
  }

  return { changed: true, skipped: false, reason: `Updated ${updates.length} manifest(s)` }
}

function requireEnvironmentVariable(name) {
  const value = process.env[name]?.trim()
  if (!value) throw new Error(`${name} is required`)
  return value
}

function readReleaseNotes({ releaseTag, repository }) {
  try {
    return execFileSync(
      "gh",
      ["release", "view", releaseTag, "--repo", repository, "--json", "body", "--jq", ".body"],
      { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] }
    )
  } catch {
    throw new Error(`Failed to read release body for ${releaseTag}`)
  }
}

function main() {
  const releaseTag = requireEnvironmentVariable("RELEASE_TAG")
  const repository = requireEnvironmentVariable("GITHUB_REPOSITORY")
  const manifestPaths = process.argv.slice(2)
  const strictVersionMatch = process.env.STRICT_VERSION_MATCH === "true"
  const releaseNotes = readReleaseNotes({ releaseTag, repository })
  const result = synchronizeManifestNotes({
    manifestPaths,
    releaseTag,
    releaseNotes,
    strictVersionMatch,
  })

  writeGithubOutput("changed", result.changed)
  writeGithubOutput("skipped", result.skipped)
  writeSummary(
    summaryLines({
      releaseTag,
      status: result.skipped ? "skipped" : result.changed ? "updated" : "unchanged",
      reason: result.reason,
    })
  )

  process.stdout.write(`${result.reason}\n`)
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  main()
}
