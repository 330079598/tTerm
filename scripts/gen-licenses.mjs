#!/usr/bin/env node
// Collects the license notices of every third-party package that ships in tTerm
// into public/licenses.json, which the About dialog shows under Open Source
// Licenses. MIT, BSD, ISC and Apache all ask that their notice travel with the
// binary, so the build runs this before bundling the frontend.
//
//   node scripts/gen-licenses.mjs [--force]
//
// Rust crates come from `cargo metadata` for every target the release builds,
// following normal dependencies only (build and dev dependencies do not ship).
// npm packages come from `pnpm licenses list --prod`. Each package's own
// LICENSE/COPYING/NOTICE files are used as is; a package without any falls
// back to the standard SPDX text of the licenses it declares.
//
// The output is skipped when it is newer than both lockfiles and this script,
// unless --force is given.

import { execFileSync } from "node:child_process"
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { createRequire } from "node:module"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..")
const OUTPUT = join(ROOT, "public", "licenses.json")
const CARGO_MANIFEST = join(ROOT, "src-tauri", "Cargo.toml")
const INPUTS = [
  join(ROOT, "src-tauri", "Cargo.lock"),
  join(ROOT, "pnpm-lock.yaml"),
  fileURLToPath(import.meta.url),
]
// The release matrix in .github/workflows; a crate used on any of them is listed.
const RUST_TARGETS = [
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "x86_64-unknown-linux-gnu",
]
// Dev dependencies whose code still lands in the bundle: Tailwind's preflight
// CSS and Vite's module preload helper.
const BUNDLED_DEV_PACKAGES = ["tailwindcss", "vite"]
const LICENSE_FILE = /^(licen[cs]e|copying|notice|copyright|unlicense)/i
const MAX_LICENSE_FILE_BYTES = 256 * 1024

const spdx = createRequire(import.meta.url)("spdx-license-list/full")

function isFresh() {
  if (!existsSync(OUTPUT)) {
    return false
  }
  const outputTime = statSync(OUTPUT).mtimeMs
  return INPUTS.every((input) => !existsSync(input) || statSync(input).mtimeMs <= outputTime)
}

function run(command, args) {
  return execFileSync(command, args, {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 1 << 28,
    // pnpm is a .cmd shim on Windows, which only a shell can start.
    shell: process.platform === "win32" && command === "pnpm",
    stdio: ["ignore", "pipe", "inherit"],
  })
}

const texts = []
const textIndex = new Map()

function addText(text) {
  const normalized = text
    .replace(/\r\n?/g, "\n")
    .replace(/[ \t]+$/gm, "")
    .trim()
  let index = textIndex.get(normalized)
  if (index === undefined) {
    index = texts.length
    texts.push(normalized)
    textIndex.set(normalized, index)
  }
  return index
}

function readLicenseFiles(dir, extraFiles = []) {
  if (!existsSync(dir)) {
    return []
  }
  const names = new Set(readdirSync(dir).filter((name) => LICENSE_FILE.test(name)))
  for (const file of extraFiles) {
    names.add(file)
  }
  const files = []
  for (const name of [...names].sort()) {
    const path = join(dir, name)
    if (!existsSync(path)) {
      continue
    }
    const stat = statSync(path)
    if (!stat.isFile() || stat.size === 0 || stat.size > MAX_LICENSE_FILE_BYTES) {
      continue
    }
    files.push({ name, text: addText(readFileSync(path, "utf8")) })
  }
  return files
}

// "MIT/Apache-2.0" and "(MIT OR Apache-2.0) AND Unicode-3.0" both name their
// licenses as plain SPDX ids between operators.
function spdxIds(expression) {
  return [
    ...new Set(
      (expression ?? "")
        .split(/[\s()/]+|\bOR\b|\bAND\b/)
        .filter((id) => id && id !== "OR" && id !== "AND" && id !== "WITH" && spdx[id])
    ),
  ]
}

function spdxFiles(expression) {
  return spdxIds(expression).map((id) => ({
    name: id,
    spdx: true,
    text: addText(spdx[id].licenseText),
  }))
}

function normalizeExpression(expression) {
  return (expression ?? "")
    .replace(/\s*\/\s*/g, " OR ")
    .replace(/\s+/g, " ")
    .trim()
}

function collectRustPackages() {
  const packages = new Map()
  for (const target of RUST_TARGETS) {
    // Not --locked: the beta release rewrites the version in Cargo.toml without
    // touching Cargo.lock, and cargo only has to update tTerm's own entry there,
    // as the cargo build that follows does anyway.
    const metadata = JSON.parse(
      run("cargo", [
        "metadata",
        "--format-version",
        "1",
        "--filter-platform",
        target,
        "--manifest-path",
        CARGO_MANIFEST,
      ])
    )
    const byId = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]))
    const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]))
    const root = metadata.resolve.root
    const seen = new Set()
    const stack = [root]
    while (stack.length > 0) {
      const id = stack.pop()
      if (seen.has(id)) {
        continue
      }
      seen.add(id)
      for (const dep of nodes.get(id).deps) {
        if (dep.dep_kinds.some((kind) => kind.kind === null)) {
          stack.push(dep.pkg)
        }
      }
    }
    seen.delete(root)
    for (const id of seen) {
      const pkg = byId.get(id)
      packages.set(`${pkg.name}@${pkg.version}`, pkg)
    }
  }

  return [...packages.values()].map((pkg) => {
    const license = normalizeExpression(pkg.license) || "See license file"
    const dir = dirname(pkg.manifest_path)
    const files = readLicenseFiles(dir, pkg.license_file ? [pkg.license_file] : [])
    return {
      name: pkg.name,
      version: pkg.version,
      license,
      source: "rust",
      url: pkg.repository || pkg.homepage || `https://crates.io/crates/${pkg.name}`,
      files: files.length > 0 ? files : spdxFiles(license),
    }
  })
}

function collectNpmPackages() {
  const byLicense = JSON.parse(run("pnpm", ["licenses", "list", "--prod", "--json"]))
  const packages = []
  for (const entries of Object.values(byLicense)) {
    for (const entry of entries) {
      entry.versions.forEach((version, i) => {
        const license = normalizeExpression(entry.license)
        const files = readLicenseFiles(entry.paths[i])
        packages.push({
          name: entry.name,
          version,
          license,
          source: "npm",
          url: entry.homepage || `https://www.npmjs.com/package/${entry.name}`,
          files: files.length > 0 ? files : spdxFiles(license),
        })
      })
    }
  }
  for (const name of BUNDLED_DEV_PACKAGES) {
    const dir = join(ROOT, "node_modules", name)
    const manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"))
    const license = normalizeExpression(manifest.license)
    const files = readLicenseFiles(dir)
    packages.push({
      name,
      version: manifest.version,
      license,
      source: "npm",
      url: manifest.homepage || `https://www.npmjs.com/package/${name}`,
      files: files.length > 0 ? files : spdxFiles(license),
    })
  }
  return packages
}

// Shipped code that no package manager reports.
function collectOtherPackages() {
  return [
    {
      name: "Rust standard library",
      version: "",
      license: "MIT OR Apache-2.0",
      source: "other",
      url: "https://github.com/rust-lang/rust",
      files: spdxFiles("MIT OR Apache-2.0"),
    },
    {
      name: "SQLite",
      version: "",
      license: "Public Domain",
      source: "other",
      url: "https://sqlite.org/copyright.html",
      files: [
        {
          name: "Blessing",
          text: addText(
            [
              "The author disclaims copyright to this source code. In place of a legal notice, here is a blessing:",
              "",
              "May you do good and not evil.",
              "May you find forgiveness for yourself and forgive others.",
              "May you share freely, never taking more than you give.",
            ].join("\n")
          ),
        },
      ],
    },
    {
      name: "iTerm2-Color-Schemes",
      version: "",
      license: "MIT",
      source: "other",
      url: "https://github.com/mbadolato/iTerm2-Color-Schemes",
      files: readLicenseFiles(join(ROOT, "src", "assets", "themes")),
    },
  ]
}

if (!process.argv.includes("--force") && isFresh()) {
  process.exit(0)
}

const started = Date.now()
const packages = [...collectRustPackages(), ...collectNpmPackages(), ...collectOtherPackages()]
packages.sort(
  (a, b) =>
    a.name.localeCompare(b.name, "en", { sensitivity: "base" }) ||
    a.version.localeCompare(b.version, "en", { numeric: true })
)

const missing = packages.filter((pkg) => pkg.files.length === 0)
for (const pkg of missing) {
  console.warn(
    `[licenses] no license text for ${pkg.source} ${pkg.name}@${pkg.version} (${pkg.license})`
  )
}

mkdirSync(dirname(OUTPUT), { recursive: true })
writeFileSync(OUTPUT, JSON.stringify({ packages, texts }))
console.log(
  `[licenses] ${packages.length} packages, ${texts.length} texts, ${Math.round(
    statSync(OUTPUT).size / 1024
  )} KB in ${Date.now() - started} ms`
)
