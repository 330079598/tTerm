#!/usr/bin/env node
// Runs the 30-session release measurements: foreground frame budget and
// background throughput.
//
//   node scripts/perf/gpui-release-frame-budget.mjs
//
// The frame fixture times wall-clock frames while 30 background parsers
// saturate the machine. The throughput fixture uses 30 fresh parsers, sends
// exactly 1 MiB through each, and requires all 30 MiB to be applied and
// acknowledged before a fixed 500 ms deadline. Both are `#[ignore]`d for the
// default suite and belong on a machine that runs nothing else. This script is
// the single entry point for local and CI runs: it picks the pinned toolchain,
// builds the release lib test binary once, lists each fixture by its full name,
// runs each alone, and validates that fixture's own evidence line. Neither
// measurement reads the other's samples.
//
// It fails closed. A renamed or misspelled fixture, a filter that matches
// nothing or more than one test, a wrong libtest mode, a test that did not run,
// a missing or malformed evidence line, a missing measurement, and a failed
// assertion all exit non-zero. A green run means both measurements ran alone,
// each reported exactly one passed test, and each evidence line is complete.
//
// `--test-name <name>` replaces the frame fixture name for one diagnostic run.
// It exists to show the failure path (see the report for W0-05); CI never
// passes it, and the other fixture still has to run.

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
const FRAME_TEST =
  "terminal_demo::pipeline::tests::thirty_background_parsers_keep_the_foreground_within_its_frame_budget"
const THROUGHPUT_TEST =
  "terminal_demo::pipeline::throughput_tests::thirty_sessions_apply_and_acknowledge_one_mebibyte_each_within_the_deadline"
const FRAME_EVIDENCE = "P2-02 foreground frames:"
const THROUGHPUT_EVIDENCE = "P2-02 background throughput:"

const EXPECTED_SESSIONS = 30
const FRAME_DEADLINE_US = 16_667
const THROUGHPUT_DEADLINE_US = 500_000
const THROUGHPUT_BYTES = EXPECTED_SESSIONS * 1024 * 1024
const MIN_THROUGHPUT_MIB_S = 60
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

/** Parses the `key=value` fields of one evidence line. */
export function evidenceFields(line) {
  return new Map(
    [...line.matchAll(/\b([a-z][a-z0-9_]*)=([^\s;]+)/g)].map(([, key, value]) => [
      key,
      value,
    ])
  )
}

function evidenceNumber(fields, key) {
  const raw = fields.get(key)
  if (raw === undefined) {
    throw new Error(`the evidence line has no ${key}`)
  }
  const value = Number(raw)
  if (!Number.isFinite(value)) {
    throw new Error(`the evidence ${key} is not a number: ${raw}`)
  }
  return value
}

/** Rejects a frame result that did not sample real background progress. */
export function validateFrameEvidence(line) {
  const fields = evidenceFields(line)
  const baselineFrames = evidenceNumber(fields, "baseline_frames")
  const loadedFrames = evidenceNumber(fields, "loaded_frames")
  const p95Us = evidenceNumber(fields, "loaded_p95_us")
  const frameWindowBytes = evidenceNumber(fields, "frame_window_acked")
  const frameWindowAdvanced = evidenceNumber(fields, "frame_window_advanced")
  const warmupAdvanced = evidenceNumber(fields, "warmup_advanced")
  if (baselineFrames !== 120 || loadedFrames !== 120) {
    throw new Error(
      `the frame fixture sampled ${baselineFrames} baseline and ${loadedFrames} loaded frames, not 120 each`
    )
  }
  if (!(p95Us > 0 && p95Us <= FRAME_DEADLINE_US)) {
    throw new Error(
      `the frame p95 is ${p95Us} us, outside the 1-${FRAME_DEADLINE_US} us budget`
    )
  }
  if (!(frameWindowBytes > 0)) {
    throw new Error("the frame window processed no background bytes")
  }
  if (!(frameWindowAdvanced > 0)) {
    throw new Error("the frame window had no background session advance")
  }
  if (warmupAdvanced !== EXPECTED_SESSIONS) {
    throw new Error(
      `the warm-up moved ${warmupAdvanced} of ${EXPECTED_SESSIONS} background sessions`
    )
  }
  return { baselineFrames, loadedFrames, p95Us, frameWindowBytes, frameWindowAdvanced, warmupAdvanced }
}

/** Rejects a throughput result that did not move and apply exactly 30 MiB. */
export function validateThroughputEvidence(line) {
  const fields = evidenceFields(line)
  const sessions = evidenceNumber(fields, "sessions")
  const bytes = evidenceNumber(fields, "bytes")
  const applied = evidenceNumber(fields, "applied")
  const acknowledged = evidenceNumber(fields, "acknowledged")
  const parsed = evidenceNumber(fields, "parsed")
  const wallUs = evidenceNumber(fields, "wall_us")
  if (sessions !== EXPECTED_SESSIONS) {
    throw new Error(`the throughput fixture ran ${sessions} sessions, not ${EXPECTED_SESSIONS}`)
  }
  if (bytes !== THROUGHPUT_BYTES) {
    throw new Error(`the throughput fixture moved ${bytes} bytes, not ${THROUGHPUT_BYTES}`)
  }
  if (applied !== bytes || acknowledged !== bytes || parsed !== bytes) {
    throw new Error(
      `the throughput fixture applied ${applied}, acknowledged ${acknowledged} and parsed ${parsed} of ${bytes} bytes`
    )
  }
  if (!(wallUs > 0 && wallUs <= THROUGHPUT_DEADLINE_US)) {
    throw new Error(
      `the 30 MiB transfer took ${wallUs} us, past the ${THROUGHPUT_DEADLINE_US} us deadline`
    )
  }
  const rateMiBs = bytes / (1024 * 1024) / (wallUs / 1_000_000)
  if (rateMiBs + 1e-9 < MIN_THROUGHPUT_MIB_S) {
    throw new Error(`the throughput rate is ${rateMiBs.toFixed(2)} MiB/s, below ${MIN_THROUGHPUT_MIB_S}`)
  }
  return { sessions, bytes, applied, acknowledged, parsed, wallUs, rateMiBs }
}

/** The two independent measurements, in the order the gate runs them. */
export const FIXTURES = [
  {
    key: "frame",
    name: FRAME_TEST,
    evidence: FRAME_EVIDENCE,
    validate: validateFrameEvidence,
  },
  {
    key: "throughput",
    name: THROUGHPUT_TEST,
    evidence: THROUGHPUT_EVIDENCE,
    validate: validateThroughputEvidence,
  },
]

/** Applies the diagnostic override without dropping the second measurement. */
export function fixturesForRun(argv) {
  const fixtures = FIXTURES.map((fixture) => ({ ...fixture }))
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] !== "--test-name") {
      throw new Error(`unknown argument: ${argv[index]}`)
    }
    const name = argv[index + 1]
    if (!name || name.startsWith("--")) {
      throw new Error("--test-name needs a value")
    }
    console.warn(
      `release gate: OVERRIDE, running "${name}" instead of the frame fixture; never use this for the gate`
    )
    fixtures[0] = { ...fixtures[0], name }
    index += 1
  }
  const names = fixtures.map((fixture) => fixture.name)
  if (new Set(names).size !== names.length) {
    throw new Error(`the release gate has duplicate fixtures: ${JSON.stringify(names)}`)
  }
  return fixtures
}

/** Requires one result per fixture, in order, so running half cannot pass. */
export function checkResults(results, fixtures) {
  if (results.length !== fixtures.length) {
    throw new Error(`ran ${results.length} of ${fixtures.length} release measurements`)
  }
  for (const [index, fixture] of fixtures.entries()) {
    if (results[index]?.key !== fixture.key) {
      throw new Error(
        `release measurement ${index} is ${results[index]?.key ?? "missing"}, expected ${fixture.key}`
      )
    }
  }
  return results
}

/**
 * Returns the counts of a run that executed exactly one test and passed it.
 * Throws with libtest's own numbers otherwise: a filter that matched nothing
 * reports zero passed, a renamed test is filtered out, and an assertion
 * failure reports FAILED. The returned `evidence` is the fixture's own line.
 */
export function checkSinglePassingRun(output, testName, evidencePrefix) {
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
  const evidence = output
    .split("\n")
    .find((line) => line.startsWith(evidencePrefix))
  if (!evidence) {
    throw new Error(`the run printed no "${evidencePrefix}" line`)
  }
  return { ...counts, evidence }
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

function run(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: MAX_OUTPUT,
    ...options,
  })
}

function main(argv) {
  const fixtures = fixturesForRun(argv)
  const rust = rustcVersion()
  console.log(
    `release gate: ${PACKAGE} ${fixtures.map((fixture) => fixture.key).join(" + ")}\n` +
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

  // Listing each ignored test with its exact name separates "the fixture is
  // gone or renamed" from "the fixture ran and failed": libtest exits zero for
  // both an empty list and an empty run.
  for (const fixture of fixtures) {
    const list = run(executable, [
      "--list",
      "--format",
      "terse",
      "--ignored",
      "--exact",
      fixture.name,
    ])
    if (list.error) throw list.error
    if (list.status !== 0) {
      throw new Error(`libtest --list exited with ${list.status}`)
    }
    const listed = listedTests(list.stdout)
    if (listed.length !== 1 || listed[0] !== fixture.name) {
      throw new Error(
        `expected exactly one ignored ${fixture.key} test named ${fixture.name}, listed ${JSON.stringify(listed)}`
      )
    }
  }

  const results = []
  const failures = []
  for (const fixture of fixtures) {
    console.log(`running the ${fixture.key} fixture alone`)
    try {
      const result = run(
        executable,
        [fixture.name, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
        { timeout: RUN_TIMEOUT_MS }
      )
      if (result.error) {
        throw result.error.code === "ETIMEDOUT"
          ? new Error(`the fixture did not finish within ${RUN_TIMEOUT_MS / 60_000} minutes`)
          : result.error
      }
      process.stdout.write(result.stdout ?? "")
      process.stderr.write(result.stderr ?? "")
      // The fixtures print their evidence with eprintln!, so it arrives on
      // stderr while libtest's result line is on stdout. Validate the run as a
      // whole rather than dropping half of its output.
      const output = (result.stdout ?? "") + (result.stderr ?? "")
      const counts = checkSinglePassingRun(output, fixture.name, fixture.evidence)
      const evidence = fixture.validate(counts.evidence)
      if (result.status !== 0) {
        throw new Error(
          `libtest exited with ${result.status} although it reported ok (${counts.passed} passed)`
        )
      }
      results.push({ key: fixture.key, name: fixture.name, counts, evidence })
      console.log(`${fixture.key}: ${JSON.stringify(evidence)}`)
    } catch (error) {
      failures.push(`${fixture.key}: ${error.message}`)
    }
  }
  if (failures.length > 0) {
    throw new Error(failures.join("; "))
  }
  checkResults(results, fixtures)
  console.log(
    `release gate: passed, ${results.length} measurements (${results.map((result) => result.key).join(", ")})`
  )
}

// Importing this file only exposes the checks above for its own tests; the
// gate runs when node is pointed at it.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main(process.argv.slice(2))
  } catch (error) {
    console.error(`release gate FAILED: ${error.message}`)
    process.exitCode = 1
  }
}
