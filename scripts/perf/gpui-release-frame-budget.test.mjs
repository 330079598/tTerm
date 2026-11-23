// Covers the fail-closed checks of scripts/perf/gpui-release-frame-budget.mjs.
// A gate that reads libtest output loosely would go green on a renamed test or
// an empty filter, so each of those shapes is pinned here.

import assert from "node:assert/strict"
import { test } from "node:test"

import {
  checkSinglePassingRun,
  listedTests,
  testExecutable,
} from "./gpui-release-frame-budget.mjs"

const NAME =
  "terminal_demo::pipeline::tests::thirty_background_parsers_keep_the_foreground_within_its_frame_budget"

function passingRun(overrides = {}) {
  const counts = {
    passed: 1,
    failed: 0,
    ignored: 0,
    measured: 0,
    filtered: 269,
    ...overrides,
  }
  return [
    `test ${NAME} ... ok`,
    "P2-02 foreground frames: baseline p50=1ms p95=2ms; bytes=33554432",
    `test result: ok. ${counts.passed} passed; ${counts.failed} failed; ${counts.ignored} ignored; ${counts.measured} measured; ${counts.filtered} filtered out; finished in 1.20s`,
    "",
  ].join("\n")
}

test("a listed ignored test is read back by its full name", () => {
  assert.deepEqual(listedTests(`${NAME}: test\n`), [NAME])
  assert.deepEqual(listedTests(""), [])
})

test("a list line in an unknown format is rejected", () => {
  assert.throws(() => listedTests(`${NAME} (ignored)`), /cannot read/)
})

test("exactly one passing test is the only accepted run", () => {
  const counts = checkSinglePassingRun(passingRun(), NAME)
  assert.equal(counts.passed, 1)
  assert.equal(counts.filtered, 269)
})

test("a run without a result line is rejected", () => {
  assert.throws(
    () => checkSinglePassingRun(`test ${NAME} ... ok\n`, NAME),
    /no libtest result line/
  )
})

test("a filter that matched nothing is rejected", () => {
  const filtered = passingRun({ passed: 0, filtered: 270 })
  assert.throws(() => checkSinglePassingRun(filtered, NAME), /exactly one executed test/)
})

test("a run of more than one test is rejected", () => {
  assert.throws(
    () => checkSinglePassingRun(passingRun({ passed: 2 }), NAME),
    /exactly one executed test/
  )
})

test("a failed assertion is rejected even with the result line present", () => {
  const failed = passingRun({ passed: 0, failed: 1 }).replace(
    "test result: ok.",
    "test result: FAILED."
  )
  assert.throws(() => checkSinglePassingRun(failed, NAME), /reported FAILED/)
})

test("a pass without the fixture's evidence line is rejected", () => {
  const silent = passingRun().replace(/^P2-02.*\n/m, "")
  assert.throws(() => checkSinglePassingRun(silent, NAME), /printed no "P2-02/)
})

test("a pass that does not name this test is rejected", () => {
  assert.throws(
    () => checkSinglePassingRun(passingRun(), `${NAME}_renamed`),
    /did not report/
  )
})

function artifact(overrides) {
  return JSON.stringify({
    reason: "compiler-artifact",
    executable: "/target/release/deps/tterm_gpui-0123",
    target: { name: "tterm_gpui", test: true, kind: ["lib"] },
    ...overrides,
  })
}

test("more than one lib test executable is rejected", () => {
  const messages = [
    JSON.stringify({ reason: "compiler-artifact", target: { name: "tterm-core" } }),
    artifact({}),
    artifact({ executable: "/target/release/deps/tterm_gpui-0456" }),
    JSON.stringify({ reason: "build-finished", success: true }),
  ].join("\n")
  assert.throws(() => testExecutable(messages), /found 2/)
})

test("exactly one lib test executable is required", () => {
  assert.equal(
    testExecutable(artifact({})),
    "/target/release/deps/tterm_gpui-0123"
  )
  assert.throws(
    () => testExecutable(JSON.stringify({ reason: "build-finished", success: true })),
    /found 0/
  )
  assert.throws(
    () => testExecutable(artifact({ target: { name: "tterm_gpui", test: true, kind: ["bin"] } })),
    /found 0/
  )
})
