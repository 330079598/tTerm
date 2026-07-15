import { appendFileSync, writeFileSync } from "node:fs"
import { execFileSync } from "node:child_process"
import { randomUUID } from "node:crypto"
import { pathToFileURL } from "node:url"

const CATEGORY_ORDER = [
  "Features",
  "Fixes",
  "Performance",
  "UI and usability",
  "Security",
  "Other changes",
]

const GITMOJI_CATEGORIES = new Map([
  ["sparkles", "Features"],
  ["bug", "Fixes"],
  ["ambulance", "Fixes"],
  ["zap", "Performance"],
  ["lipstick", "UI and usability"],
  ["art", "UI and usability"],
  ["lock", "Security"],
])

const EXCLUDED_GITMOJI = new Set([
  "bookmark",
  "building_construction",
  "construction_worker",
  "green_heart",
  "package",
  "pushpin",
  "recycle",
  "see_no_evil",
  "test_tube",
  "twisted_rightwards_arrows",
  "white_check_mark",
  "wrench",
])

const CONVENTIONAL_CATEGORIES = new Map([
  ["feat", "Features"],
  ["fix", "Fixes"],
  ["perf", "Performance"],
  ["security", "Security"],
])

const EXCLUDED_CONVENTIONAL_TYPES = new Set([
  "build",
  "chore",
  "ci",
  "docs",
  "refactor",
  "style",
  "test",
])

export function classifyCommit(subject) {
  const gitmojiMatch = subject.match(/^:([a-z0-9_+-]+):\s*(.+)$/i)
  if (gitmojiMatch) {
    const type = gitmojiMatch[1].toLowerCase()
    if (EXCLUDED_GITMOJI.has(type)) return null
    return {
      category: GITMOJI_CATEGORIES.get(type) ?? "Other changes",
      description: gitmojiMatch[2].trim(),
    }
  }

  const conventionalMatch = subject.match(/^([a-z]+)(?:\([^)]+\))?(!)?:\s*(.+)$/i)
  if (conventionalMatch) {
    const type = conventionalMatch[1].toLowerCase()
    if (EXCLUDED_CONVENTIONAL_TYPES.has(type)) return null
    return {
      category: CONVENTIONAL_CATEGORIES.get(type) ?? "Other changes",
      description: conventionalMatch[3].trim(),
    }
  }

  return { category: "Other changes", description: subject.trim() }
}

function escapeMarkdownText(value) {
  return value.replaceAll("\\", "\\\\").replaceAll("[", "\\[").replaceAll("]", "\\]")
}

export function buildReleaseNotes({ commits, previousTag, betaTag, repository }) {
  const grouped = new Map(CATEGORY_ORDER.map((category) => [category, []]))

  for (const commit of commits) {
    const classified = classifyCommit(commit.subject)
    if (!classified) continue
    grouped.get(classified.category).push({ ...commit, ...classified })
  }

  const lines = [`## Changes since \`${previousTag}\``, ""]
  let includedCommitCount = 0

  for (const category of CATEGORY_ORDER) {
    const categoryCommits = grouped.get(category)
    if (categoryCommits.length === 0) continue

    lines.push(`### ${category}`, "")
    for (const commit of categoryCommits) {
      const shortSha = commit.sha.slice(0, 8)
      const commitUrl = `https://github.com/${repository}/commit/${commit.sha}`
      lines.push(`- ${escapeMarkdownText(commit.description)} ([${shortSha}](${commitUrl}))`)
      includedCommitCount += 1
    }
    lines.push("")
  }

  if (includedCommitCount === 0) {
    lines.push("_No user-facing changes were detected in this build._", "")
  }

  const compareUrl = `https://github.com/${repository}/compare/${previousTag}...${betaTag}`
  lines.push(`**Full comparison:** [${previousTag}...${betaTag}](${compareUrl})`, "")

  return lines.join("\n")
}

function readCommits(previousTag) {
  const output = execFileSync(
    "git",
    ["log", "--no-merges", "--format=%H%x1f%s%x1e", `${previousTag}..HEAD`],
    { encoding: "utf8" }
  )

  return output
    .split("\x1e")
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const separator = entry.indexOf("\x1f")
      if (separator === -1) throw new Error(`Unable to parse git log entry: ${entry}`)
      return {
        sha: entry.slice(0, separator),
        subject: entry.slice(separator + 1),
      }
    })
}

function requireEnvironmentVariable(name) {
  const value = process.env[name]?.trim()
  if (!value) throw new Error(`${name} is required`)
  return value
}

function main() {
  const previousTag = requireEnvironmentVariable("PREVIOUS_TAG")
  const betaTag = requireEnvironmentVariable("BETA_TAG")
  const repository = requireEnvironmentVariable("GITHUB_REPOSITORY")
  const githubOutput = requireEnvironmentVariable("GITHUB_OUTPUT")
  const commits = readCommits(previousTag)
  const releaseNotes = buildReleaseNotes({
    commits,
    previousTag,
    betaTag,
    repository,
  })

  writeFileSync("release-notes.md", releaseNotes, "utf8")

  const delimiter = `release_notes_${randomUUID()}`
  appendFileSync(
    githubOutput,
    `release_notes<<${delimiter}\n${releaseNotes}\n${delimiter}\n`,
    "utf8"
  )

  process.stdout.write(`${releaseNotes}\n`)
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  main()
}
