import fs from "node:fs/promises"
import path from "node:path"

const [rawDir, output] = process.argv.slice(2)
if (!rawDir || !output) throw new Error("Usage: node summarize.mjs RAW_DIRECTORY OUTPUT.csv")
const header = ["平台", "构建", "负载", "指标", "数值", "单位", "样本数", "备注"]
const csv = [header]
const grouped = new Map()
const mean = (values) => values.reduce((sum, value) => sum + value, 0) / values.length
const columns = {
  cpu_percent_machine: "% whole machine",
  working_set_bytes: "bytes; summed RSS includes shared pages per process",
  private_working_set_bytes: "bytes",
  private_bytes: "bytes committed",
  gpu_dedicated_bytes: "bytes",
  gpu_shared_bytes: "bytes",
}
for (const filename of (await fs.readdir(rawDir))
  .filter((f) => /^run[12]-.+\.json$/.test(f))
  .sort()) {
  const raw = JSON.parse(
    (await fs.readFile(path.join(rawDir, filename), "utf8")).replace(/^\uFEFF/, "")
  )
  if (!Array.isArray(raw.samples)) continue
  const run = filename.match(/^run([12])/)[1]
  for (const [column, unit] of Object.entries(columns)) {
    // The first PDH sample initializes rate counters and is excluded consistently.
    const values = raw.samples
      .slice(1)
      .map((s) => s[column])
      .filter((v) => v !== null && Number.isFinite(v))
    for (const [suffix, operation] of [
      ["mean", mean],
      ["peak", (values) => Math.max(...values)],
    ]) {
      const metric = `${column}_${suffix}`
      const value = values.length ? operation(values) : ""
      csv.push([
        "windows",
        raw.build,
        raw.workload,
        metric,
        value,
        unit,
        values.length,
        `run=${run}; ConPTY; raw=${filename}; skipped_queries=${raw.skipped_queries || 0}; ${values.length ? "" : raw.gpu_error || "unavailable"}`,
      ])
      if (values.length) {
        const key = `${raw.workload}|${metric}`
        const pair = grouped.get(key) || {}
        pair[run] = value
        grouped.set(key, pair)
      }
    }
  }
}
const comparisons = []
for (const [key, pair] of grouped) {
  if (pair[1] === undefined || pair[2] === undefined) continue
  const difference =
    pair[1] === 0
      ? pair[2] === 0
        ? 0
        : null
      : (Math.abs(pair[2] - pair[1]) / Math.abs(pair[1])) * 100
  const [load, metric] = key.split("|")
  comparisons.push({ load, metric, run1: pair[1], run2: pair[2], difference_percent: difference })
  csv.push([
    "windows",
    "release-tauri-perf",
    load,
    `${metric}_difference`,
    difference ?? "",
    "% versus run1",
    2,
    difference === null
      ? "run1=0; relative difference undefined"
      : difference > 10
        ? "超过10%；噪声分析见报告"
        : "within 10%",
  ])
}
csv.push([
  "windows",
  "release-tauri-perf",
  "vtebench-standard",
  "pending",
  "",
  "",
  0,
  "待补（WSL 缺少 Rust 工具链）",
])
const quote = (value) => `"${String(value).replaceAll('"', '""')}"`
await fs.writeFile(output, csv.map((row) => row.map(quote).join(",")).join("\n") + "\n")
await fs.writeFile(
  path.join(rawDir, "comparison.json"),
  JSON.stringify(comparisons, null, 2) + "\n"
)
console.log(`Wrote ${csv.length - 1} rows to ${output}`)
