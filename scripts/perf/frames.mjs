import fs from "node:fs/promises"
import path from "node:path"
import { gunzipSync } from "node:zlib"
import { createHash } from "node:crypto"

const [directory, output] = process.argv.slice(2)
if (!directory || !output) throw new Error("Usage: node frames.mjs RAW_DIRECTORY OUTPUT.csv")
const rows = [["平台", "构建", "负载", "指标", "数值", "单位", "样本数", "备注"]]
const summaries = []
const inventory = await fs.readdir(directory)
const traces = inventory.filter((f) => /^run[12]-.+\.trace\.json\.gz$/.test(f))
const files = traces.length ? traces : inventory.filter((f) => /^run[12]-.+\.frames\.json$/.test(f))
for (const file of files.sort()) {
  const source = await fs.readFile(path.join(directory, file))
  const full = file.endsWith(".gz")
  const trace = JSON.parse(full ? gunzipSync(source) : source)
  const evidence = trace.traceEvents.filter(
    (event) =>
      event.name === "DrawFrame" || event.name === "process_name" || event.name === "thread_name"
  )
  if (full)
    await fs.writeFile(
      path.join(directory, file.replace(".trace.json.gz", ".frames.json")),
      JSON.stringify({
        source: file,
        sha256: createHash("sha256").update(source).digest("hex"),
        traceEvents: evidence,
      }) + "\n"
    )
  const groups = new Map()
  for (const event of trace.traceEvents) {
    if (event.name !== "DrawFrame" || event.ph !== "I") continue
    const key = `${event.pid}/${event.tid}`
    const timestamps = groups.get(key) || []
    timestamps.push(event.ts)
    groups.set(key, timestamps)
  }
  const sorted = [...groups.entries()].sort((a, b) => b[1].length - a[1].length)
  const [key, times] = sorted[0] || ["", []]
  times.sort((a, b) => a - b)
  const intervals = times
    .slice(1)
    .map((time, i) => (time - times[i]) / 1000)
    .filter((t) => t > 0)
    .sort((a, b) => a - b)
  const load = file.replace(/^run[12]-/, "").replace(/\.(trace\.json\.gz|frames\.json)$/, "")
  const run = file.match(/^run([12])/)[1]
  const percentile = (p) => intervals[Math.max(0, Math.ceil(intervals.length * p) - 1)]
  const metrics = intervals.length
    ? {
        frame_interval_p50: percentile(0.5),
        frame_interval_p95: percentile(0.95),
        frame_interval_p99: percentile(0.99),
        frame_interval_max: intervals.at(-1),
        frame_interval_over_16_667ms_percent:
          (intervals.filter((t) => t > 16.667).length / intervals.length) * 100,
      }
    : { frame_interval_unavailable: "" }
  const note = `run=${run}; DrawFrame ph=I pid/tid=${key}; trace=${file}; idle gaps are not stalls`
  for (const [metric, value] of Object.entries(metrics)) {
    rows.push([
      "windows",
      "release-tauri-perf",
      load,
      metric,
      value,
      metric.endsWith("percent") ? "%" : "ms",
      intervals.length,
      note,
    ])
  }
  summaries.push({ run, load, pid_tid: key, samples: intervals.length, metrics })
}
for (const first of summaries.filter((s) => s.run === "1")) {
  const second = summaries.find((s) => s.run === "2" && s.load === first.load)
  if (!second) continue
  for (const [metric, value] of Object.entries(first.metrics)) {
    const delta = value > 0 ? (Math.abs(second.metrics[metric] - value) / value) * 100 : ""
    rows.push([
      "windows",
      "release-tauri-perf",
      first.load,
      `${metric}_difference`,
      delta,
      "% versus run1",
      2,
      delta > 10 ? "超过10%；噪声分析见报告" : "see raw traces",
    ])
  }
}
const quote = (value) => `"${String(value).replaceAll('"', '""')}"`
await fs.writeFile(output, rows.map((row) => row.map(quote).join(",")).join("\n") + "\n")
await fs.writeFile(
  path.join(directory, "frames-summary.json"),
  JSON.stringify(summaries, null, 2) + "\n"
)
console.log(`Wrote ${rows.length - 1} frame rows`)
