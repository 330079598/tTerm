// Covers the fail-closed checks of scripts/perf/gpui-release-frame-budget.mjs.
// A gate that reads libtest output loosely would go green on a renamed test,
// an empty filter, a missing measurement or an evidence line without numbers,
// so each of those shapes is pinned here.

import assert from "node:assert/strict"
import { test } from "node:test"

import {
  FIXTURES,
  checkResults,
  checkSinglePassingRun,
  evidenceFields,
  fixturesForRun,
  listedTests,
  testExecutable,
  validateFrameEvidence,
  validateThroughputEvidence,
} from "./gpui-release-frame-budget.mjs"

const FRAME = FIXTURES.find((fixture) => fixture.key === "frame")
const THROUGHPUT = FIXTURES.find((fixture) => fixture.key === "throughput")
const FRAME_EVIDENCE =
  "P2-02 foreground frames: baseline p50=113µs p95=118µs; 30-background p50=111µs p95=128µs; " +
  "snapshots=253 frames=253; baseline_frames=120 loaded_frames=120 loaded_p95_us=128; " +
  "bytes=26404000; warmup_acked=23616000; warmup_advanced=30; frame_window=13.946ms; " +
  "frame_window_acked=2788000 (190.65 MiB/s); frame_window_advanced=30; " +
  "snapshot_wait=2.376µs total, snapshot_hold=6.93ms total / 84.125µs max over 253 snapshots"
const THROUGHPUT_EVIDENCE =
  "P2-02 background throughput: sessions=30 bytes=31457280 applied=31457280 " +
  "acknowledged=31457280 parsed=31457280 wall_us=200000 rate_mib_s=150.00 " +
  "deadline_us=500000 completed=true"

function passingRun(fixture, overrides = {}) {
  const counts = {
    passed: 1,
    failed: 0,
    ignored: 0,
    measured: 0,
    filtered: 269,
    ...overrides,
  }
  return [
    `test ${fixture.name} ... ok`,
    fixture.key === "frame" ? FRAME_EVIDENCE : THROUGHPUT_EVIDENCE,
    `test result: ok. ${counts.passed} passed; ${counts.failed} failed; ${counts.ignored} ignored; ${counts.measured} measured; ${counts.filtered} filtered out; finished in 1.20s`,
    "",
  ].join("\n")
}

test("a listed ignored test is read back by its full name", () => {
  assert.deepEqual(listedTests(`${FRAME.name}: test\n`), [FRAME.name])
  assert.deepEqual(listedTests(""), [])
})

test("a list line in an unknown format is rejected", () => {
  assert.throws(() => listedTests(`${FRAME.name} (ignored)`), /cannot read/)
})

test("exactly one passing test is the only accepted run", () => {
  const counts = checkSinglePassingRun(
    passingRun(FRAME),
    FRAME.name,
    FRAME.evidence
  )
  assert.equal(counts.passed, 1)
  assert.equal(counts.filtered, 269)
  assert.match(counts.evidence, /^P2-02 foreground frames:/)
})

test("the throughput fixture has its own passing run and evidence", () => {
  const counts = checkSinglePassingRun(
    passingRun(THROUGHPUT),
    THROUGHPUT.name,
    THROUGHPUT.evidence
  )
  assert.equal(counts.passed, 1)
  assert.match(counts.evidence, /^P2-02 background throughput:/)
})

test("a run without a result line is rejected", () => {
  assert.throws(
    () => checkSinglePassingRun(`test ${FRAME.name} ... ok\n`, FRAME.name, FRAME.evidence),
    /no libtest result line/
  )
})

test("a filter that matched nothing is rejected", () => {
  const filtered = passingRun(FRAME, { passed: 0, filtered: 270 })
  assert.throws(
    () => checkSinglePassingRun(filtered, FRAME.name, FRAME.evidence),
    /exactly one executed test/
  )
})

test("a run of more than one test is rejected", () => {
  assert.throws(
    () => checkSinglePassingRun(passingRun(FRAME, { passed: 2 }), FRAME.name, FRAME.evidence),
    /exactly one executed test/
  )
})

test("a failed assertion is rejected even with the result line present", () => {
  const failed = passingRun(FRAME, { passed: 0, failed: 1 }).replace(
    "test result: ok.",
    "test result: FAILED."
  )
  assert.throws(
    () => checkSinglePassingRun(failed, FRAME.name, FRAME.evidence),
    /reported FAILED/
  )
})

test("a pass without the fixture's evidence line is rejected", () => {
  const silent = passingRun(FRAME).replace(/^P2-02.*\n/m, "")
  assert.throws(
    () => checkSinglePassingRun(silent, FRAME.name, FRAME.evidence),
    /printed no "P2-02/
  )
})

test("a pass that does not name this test is rejected", () => {
  assert.throws(
    () =>
      checkSinglePassingRun(
        passingRun(FRAME),
        `${FRAME.name}_renamed`,
        FRAME.evidence
      ),
    /did not report/
  )
})

test("the evidence parser reads all key/value fields", () => {
  const fields = evidenceFields(FRAME_EVIDENCE)
  assert.equal(fields.get("loaded_p95_us"), "128")
  assert.equal(fields.get("warmup_advanced"), "30")
  assert.equal(fields.get("frame_window_acked"), "2788000")
})

test("a complete frame evidence line is accepted", () => {
  const evidence = validateFrameEvidence(FRAME_EVIDENCE)
  assert.equal(evidence.p95Us, 128)
  assert.equal(evidence.frameWindowAdvanced, 30)
})

test("the frame evidence rejects a missing field", () => {
  const line = FRAME_EVIDENCE.replace(" loaded_p95_us=128;", "")
  assert.throws(() => validateFrameEvidence(line), /no loaded_p95_us/)
})

test("the frame evidence rejects a p95 over the budget", () => {
  const line = FRAME_EVIDENCE.replace("loaded_p95_us=128", "loaded_p95_us=16668")
  assert.throws(() => validateFrameEvidence(line), /outside the 1-16667 us budget/)
})

test("the frame evidence rejects a wrong sample count", () => {
  const line = FRAME_EVIDENCE.replace("loaded_frames=120", "loaded_frames=119")
  assert.throws(() => validateFrameEvidence(line), /not 120 each/)
})

test("the frame evidence rejects an idle frame window", () => {
  const noBytes = FRAME_EVIDENCE.replace("frame_window_acked=2788000", "frame_window_acked=0")
  assert.throws(() => validateFrameEvidence(noBytes), /no background bytes/)
  const noProgress = FRAME_EVIDENCE.replace(
    "frame_window_advanced=30",
    "frame_window_advanced=0"
  )
  assert.throws(() => validateFrameEvidence(noProgress), /no background session advance/)
  const coldWarmup = FRAME_EVIDENCE.replace("warmup_advanced=30", "warmup_advanced=29")
  assert.throws(() => validateFrameEvidence(coldWarmup), /moved 29 of 30/)
})

test("a complete throughput evidence line is accepted", () => {
  const evidence = validateThroughputEvidence(THROUGHPUT_EVIDENCE)
  assert.equal(evidence.bytes, 30 * 1024 * 1024)
  assert.equal(evidence.wallUs, 200_000)
})

test("the throughput evidence rejects a missing session or wrong total", () => {
  assert.throws(
    () =>
      validateThroughputEvidence(
        THROUGHPUT_EVIDENCE.replace("sessions=30", "sessions=29")
      ),
    /ran 29 sessions/
  )
  assert.throws(
    () =>
      validateThroughputEvidence(
        THROUGHPUT_EVIDENCE.replace("bytes=31457280", "bytes=30408704")
      ),
    /moved 30408704 bytes/
  )
})

test("the throughput evidence rejects unapplied or unacknowledged bytes", () => {
  assert.throws(
    () =>
      validateThroughputEvidence(
        THROUGHPUT_EVIDENCE.replace("applied=31457280", "applied=31457279")
      ),
    /applied 31457279/
  )
  assert.throws(
    () =>
      validateThroughputEvidence(
        THROUGHPUT_EVIDENCE.replace("acknowledged=31457280", "acknowledged=31457279")
      ),
    /acknowledged 31457279/
  )
  assert.throws(
    () =>
      validateThroughputEvidence(
        THROUGHPUT_EVIDENCE.replace("parsed=31457280", "parsed=31457279")
      ),
    /parsed 31457279/
  )
})

test("the throughput evidence rejects a missed 500 ms deadline", () => {
  const late = THROUGHPUT_EVIDENCE.replace("wall_us=200000", "wall_us=500001")
  assert.throws(() => validateThroughputEvidence(late), /past the 500000 us deadline/)
})

test("the gate keeps both fixtures and an override never drops one", () => {
  assert.deepEqual(
    fixturesForRun([]).map((fixture) => fixture.key),
    ["frame", "throughput"]
  )
  const overridden = fixturesForRun(["--test-name", "other::frame"])
  assert.equal(overridden.length, 2)
  assert.equal(overridden[0].name, "other::frame")
  assert.equal(overridden[1].name, THROUGHPUT.name)
  assert.throws(
    () => fixturesForRun(["--test-name", THROUGHPUT.name]),
    /duplicate fixtures/
  )
  assert.throws(() => fixturesForRun(["--unknown"]), /unknown argument/)
  assert.throws(() => fixturesForRun(["--test-name"]), /needs a value/)
})

test("only one measurement cannot pass as the complete gate", () => {
  const frameResult = { key: "frame" }
  assert.throws(() => checkResults([frameResult], FIXTURES), /ran 1 of 2/)
  assert.throws(
    () => checkResults([frameResult, { key: "throughput" }], [FIXTURES[1], FIXTURES[0]]),
    /expected throughput/
  )
  assert.deepEqual(checkResults([{ key: "frame" }, { key: "throughput" }], FIXTURES), [
    { key: "frame" },
    { key: "throughput" },
  ])
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
    () =>
      testExecutable(
        artifact({ target: { name: "tterm_gpui", test: true, kind: ["bin"] } })
      ),
    /found 0/
  )
})
