#!/usr/bin/env node
// Runs the 30-session foreground frame budget as its own release measurement.
//
//   node scripts/perf/gpui-release-frame-budget.mjs
//
// The fixture times wall-clock frames while 30 background parsers saturate the
// machine, so it is `#[ignore]`d for the default suite and belongs on a machine
// that runs nothing else. This script is the single entry point for that run:
// CI's `rust-frame-budget` job and a local check call it the same way, and it
// picks the pinned toolchain, `--release`, the GPUI package and the lib test
// binary itself so neither side can drift into a different measurement.
//
// It fails closed. A renamed or misspelled test, a filter that matches nothing
// or more than one test, a wrong libtest mode, a test that did not run, and a
// failed assertion all exit non-zero. The gate is green only when libtest lists
// exactly this one ignored test, runs exactly one test, and reports it passed.
//
// `--test-name <name>` replaces the fixture name for one run. It exists to show
// the failure path (see the report for W0-05); CI never passes it.

import { execFileSync, spawnSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { availableParallelism } from "node:os"

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..")

// The benchmark is the only thing here that needs release code: the GPUI crate
// carries the terminal state machine and the view. AGENTS.md asks for `--workspace`
// on package selections; naming the one package is the deliberate exception,
// so one fixture does not compile the Tauri app and the rest of the workspace
// in release.
const PACKAGE = "tterm-gpui"
// Cargo names the artifact after the crate, with underscores.
const TARGET = PACKAGE.replaceAll("-", "_")
// libtest filters on the module path, so `--exact` needs the whole name: the
// short function name matches nothing, and listing it that way is a green exit
// code with zero tests.
const TEST_NAME =
  "terminal_demo::pipeline::tests::thirty_background_parsers_keep_the_foreground_within_its_frame_budget"
// The fixture prints this before it asserts, so a green run has a measurement
// to show and a test that stopped early cannot look like a pass.
const EVIDENCE_PREFIX = "P2-02 foreground frames:"
const MAX_OUTPUT = 64 * 1024 * 1024
const RUN_TIMEOUT_MS = 10 * 60 * 1000

/**
 * Reads the one-line-per-test output of `libtest --list`.
 * Throws on a line the gate cannot interpret, so an unexpected format cannot
 * pass as an empty or a matching list.
 */
export function listedTests(output) {
  const names = []
  for (const line of output.split("\n")) {
    const trimmed = line.trim()
    if (!trimmed) continue
    const match = /^(.+): (?:test|benchmark)$/.exec(trimmed)
    if (!match) {
      throw new Error(`libtest listed a line this gate cannot read: ${line}`)
    }
    names.push(match[1])
  }
  return names
}

/**
 * Returns the counts of a run that executed exactly one test and passed it.
 * Throws with libtest's own numbers otherwise: a filter that matched nothing
 * reports zero passed, a renamed test is filtered out, and an assertion
 * failure reports FAILED.
 */
export function checkSinglePassingRun(output, testName) {
  const result = /^test result: (\w+)\.(.*)$/m.exec(output)
  if (!result) {
    throw new Error("the run printed no libtest result line")
  }
  const counts = Object.fromEntries(
    [...result[2].matchAll(/(\d+) (passed|failed|ignored|measured|filtered out)/g)].map(
      ([, count, what]) => [what === "filtered out" ? "filtered" : what, Number(count)]
    )
  )
  const summary = `${counts.passed ?? 0} passed, ${counts.failed ?? 0} failed, ${counts.ignored ?? 0} ignored, ${counts.filtered ?? 0} filtered out`
  if (result[1] !== "ok") {
    throw new Error(`libtest reported ${result[1]}: ${summary}`)
  }
  if (counts.passed !== 1 || counts.failed !== 0 || counts.ignored !== 0) {
    throw new Error(`the run was not exactly one executed test: ${summary}`)
  }
  if (!output.includes(`test ${testName} ... ok`)) {
    throw new Error(`the run did not report ${testName} as passed`)
  }
  if (!output.includes(EVIDENCE_PREFIX)) {
    throw new Error(`the run printed no "${EVIDENCE_PREFIX}" line`)
  }
  return counts
}

/** Finds the lib test executable among cargo's JSON build messages. */
export function testExecutable(messages) {
  const executables = []
  for (const line of messages.split("\n")) {
    if (!line.startsWith("{")) continue
    let message
    try {
      message = JSON.parse(line)
    } catch {
      throw new Error(`cargo printed a build message this gate cannot read: ${line}`)
    }
    if (message.reason !== "compiler-artifact" || !message.executable) continue
    const target = message.target ?? {}
    if (target.name === TARGET && target.test && (target.kind ?? []).includes("lib")) {
      executables.push(message.executable)
    }
  }
  if (executables.length !== 1) {
    throw new Error(
      `expected one lib test executable for ${PACKAGE}, found ${executables.length}`
    )
  }
  return executables[0]
}

function gitCommit() {
  try {
    return execFileSync("git", ["rev-parse", "--short", "HEAD"], {
      cwd: ROOT,
      encoding: "utf8",
    }).trim()
  } catch {
    return "unknown"
  }
}

function pinnedChannel() {
  const pinned = readFileSync(join(ROOT, "rust-toolchain.toml"), "utf8")
  const channel = /^\s*channel\s*=\s*"([^"]+)"/m.exec(pinned)
  if (!channel) {
    throw new Error("rust-toolchain.toml does not name a channel")
  }
  return channel[1]
}

function rustcVersion() {
  const version = execFileSync("rustc", ["--version"], {
    cwd: ROOT,
    encoding: "utf8",
  }).trim()
  const channel = pinnedChannel()
  if (version.split(" ")[1] !== channel) {
    throw new Error(
      `rustc is "${version}", but rust-toolchain.toml pins ${channel}; run rustup show`
    )
  }
  return version
}

function testName(argv) {
  const index = argv.indexOf("--test-name")
  if (index === -1) return TEST_NAME
  const name = argv[index + 1]
  if (!name) throw new Error("--test-name needs a value")
  console.warn(
    `frame budget gate: OVERRIDE, running "${name}" instead of the fixture; never use this for the gate`
  )
  return name
}

function run(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: MAX_OUTPUT,
    ...options,
  })
}

function main(argv) {
  const name = testName(argv)
  const rust = rustcVersion()
  console.log(
    `frame budget gate: ${PACKAGE} ${name}\n` +
      `platform: ${process.platform} ${process.arch}, ${availableParallelism()} cpus\n` +
      `rust: ${rust}\n` +
      `commit: ${gitCommit()}\n` +
      `This measurement needs the machine to itself; do not run other work alongside it.`
  )

  console.log("building the release lib test binary")
  const build = run(
    "cargo",
    [
      "test",
      "-p",
      PACKAGE,
      "--lib",
      "--release",
      "--no-run",
      "--message-format=json-render-diagnostics",
    ],
    { stdio: ["ignore", "pipe", "inherit"] }
  )
  if (build.error) throw build.error
  if (build.status !== 0) {
    throw new Error(`cargo test --no-run exited with ${build.status}`)
  }
  const executable = testExecutable(build.stdout)

  // Listing the ignored tests with the exact name separates "the fixture is
  // gone or renamed" from "the fixture ran and failed": libtest exits zero for
  // both an empty list and an empty run.
  const list = run(executable, ["--list", "--format", "terse", "--ignored", "--exact", name])
  if (list.error) throw list.error
  if (list.status !== 0) {
    throw new Error(`libtest --list exited with ${list.status}`)
  }
  const listed = listedTests(list.stdout)
  if (listed.length !== 1 || listed[0] !== name) {
    throw new Error(
      `expected exactly one ignored test named ${name}, listed ${JSON.stringify(listed)}`
    )
  }

  console.log("running the fixture alone")
  const result = run(
    executable,
    [name, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
    { timeout: RUN_TIMEOUT_MS }
  )
  if (result.error) {
    throw result.error.code === "ETIMEDOUT"
      ? new Error(`the fixture did not finish within ${RUN_TIMEOUT_MS / 60_000} minutes`)
      : result.error
  }
  process.stdout.write(result.stdout ?? "")
  process.stderr.write(result.stderr ?? "")
  const counts = checkSinglePassingRun(result.stdout ?? "", name)
  if (result.status !== 0) {
    throw new Error(
      `libtest exited with ${result.status} although it reported ok (${counts.passed} passed)`
    )
  }
  console.log(`frame budget gate: passed, ${counts.passed} test, ${counts.filtered} filtered out`)
}

// Importing this file only exposes the checks above for its own tests; the
// gate runs when node is pointed at it.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main(process.argv.slice(2))
  } catch (error) {
    console.error(`frame budget gate FAILED: ${error.message}`)
    process.exitCode = 1
  }
}
