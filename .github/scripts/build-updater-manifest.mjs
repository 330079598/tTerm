import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { execFileSync } from "node:child_process"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { pathToFileURL } from "node:url"

// Every updater artifact the release matrix must produce, with the manifest
// platform keys tauri-action would have written for it.
const UPDATER_ARTIFACTS = [
  { label: "macOS arm64", pattern: /_aarch64\.app\.tar\.gz$/, platforms: ["darwin-aarch64", "darwin-aarch64-app"] },
  { label: "macOS x64", pattern: /_x64\.app\.tar\.gz$/, platforms: ["darwin-x86_64", "darwin-x86_64-app"] },
  { label: "Windows NSIS", pattern: /_x64-setup\.exe$/, platforms: ["windows-x86_64", "windows-x86_64-nsis"] },
  { label: "Linux AppImage", pattern: /_amd64\.AppImage$/, platforms: ["linux-x86_64", "linux-x86_64-appimage"] },
  { label: "Linux deb", pattern: /_amd64\.deb$/, platforms: ["linux-x86_64-deb"] },
  { label: "Linux rpm", pattern: /\.x86_64\.rpm$/, platforms: ["linux-x86_64-rpm"] },
]

function expectedVersion(releaseTag) {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(releaseTag)) {
    throw new Error(`Invalid release tag: ${releaseTag}`)
  }
  return releaseTag.slice(1)
}

export function buildUpdaterManifest({
  releaseTag,
  repository,
  releaseNotes,
  pubDate,
  assetNames,
  readSignature,
}) {
  if (!releaseNotes.trim()) throw new Error(`Release body is empty for ${releaseTag}`)

  const version = expectedVersion(releaseTag)
  const names = new Set(assetNames)
  const platforms = {}
  const problems = []

  for (const { label, pattern, platforms: keys } of UPDATER_ARTIFACTS) {
    const matches = assetNames.filter((name) => pattern.test(name))
    if (matches.length !== 1) {
      problems.push(
        matches.length === 0
          ? `${label}: no artifact matching ${pattern}`
          : `${label}: ${matches.length} artifacts match ${pattern} (${matches.join(", ")})`
      )
      continue
    }

    const [artifact] = matches
    if (!names.has(`${artifact}.sig`)) {
      problems.push(`${label}: ${artifact} has no ${artifact}.sig`)
      continue
    }

    const signature = readSignature(`${artifact}.sig`)
    if (!signature.trim()) {
      problems.push(`${label}: ${artifact}.sig is empty`)
      continue
    }

    const url = `https://github.com/${repository}/releases/download/${releaseTag}/${encodeURIComponent(artifact)}`
    for (const key of keys) platforms[key] = { signature, url }
  }

  if (problems.length > 0) {
    throw new Error(`Release ${releaseTag} is missing updater artifacts:\n- ${problems.join("\n- ")}`)
  }

  return { version, notes: releaseNotes, pub_date: pubDate, platforms }
}

function requireEnvironmentVariable(name) {
  const value = process.env[name]?.trim()
  if (!value) throw new Error(`${name} is required`)
  return value
}

function gh(args) {
  return execFileSync("gh", args, { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] })
}

function main() {
  const releaseTag = requireEnvironmentVariable("RELEASE_TAG")
  const repository = requireEnvironmentVariable("GITHUB_REPOSITORY")
  const outputPath = process.argv[2]
  if (!outputPath) throw new Error("Output manifest path is required")

  const release = JSON.parse(
    gh(["release", "view", releaseTag, "--repo", repository, "--json", "body,assets"])
  )
  const signatureDirectory = mkdtempSync(join(tmpdir(), "tterm-updater-signatures-"))

  try {
    gh([
      "release", "download", releaseTag,
      "--repo", repository,
      "--pattern", "*.sig",
      "--dir", signatureDirectory,
    ])

    const manifest = buildUpdaterManifest({
      releaseTag,
      repository,
      releaseNotes: release.body ?? "",
      pubDate: new Date().toISOString(),
      assetNames: release.assets.map(({ name }) => name),
      readSignature: (name) => readFileSync(join(signatureDirectory, name), "utf8"),
    })

    mkdirSync(dirname(outputPath), { recursive: true })
    writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8")

    const platformKeys = Object.keys(manifest.platforms)
    const summaryPath = process.env.GITHUB_STEP_SUMMARY
    if (summaryPath) {
      appendFileSync(
        summaryPath,
        [
          "## Updater manifest",
          "",
          `- Release: \`${releaseTag}\``,
          `- Platforms: ${platformKeys.map((key) => `\`${key}\``).join(", ")}`,
          "",
        ].join("\n"),
        "utf8"
      )
    }

    process.stdout.write(`Built ${outputPath} with ${platformKeys.length} platform entries\n`)
  } finally {
    rmSync(signatureDirectory, { recursive: true, force: true })
  }
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  main()
}
