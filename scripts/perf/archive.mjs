import fs from "node:fs/promises"
import path from "node:path"
import { gzipSync } from "node:zlib"

const [raw, destination] = process.argv.slice(2)
if (!raw || !destination) throw new Error("Usage: node archive.mjs RAW_DIRECTORY DESTINATION")
await fs.mkdir(destination, { recursive: true })
const files = await fs.readdir(raw)
for (const file of files) {
  if (/^run[12]-.+\.json$/.test(file)) {
    const bytes = await fs.readFile(path.join(raw, file))
    await fs.writeFile(path.join(destination, `${file}.gz`), gzipSync(bytes))
  } else if (
    /^run[12]-.+(active\.png|\.collector\.txt)$/.test(file) ||
    /^(conditions-run[12]|isolation-run[12]|display)\.json$/.test(file) ||
    /^(binary-hash|final-run[12]|final-run2-tail|cleanup|second-round-counter-error)\.txt$/.test(
      file
    ) ||
    /^check-.+\.txt$/.test(file)
  ) {
    await fs.copyFile(path.join(raw, file), path.join(destination, file))
  }
}
for (const run of [1, 2]) {
  const directory = path.join(raw, `load-run${run}`)
  for (const file of await fs.readdir(directory)) {
    if (/\.(done|started)$/.test(file)) {
      await fs.copyFile(path.join(directory, file), path.join(destination, `run${run}-${file}`))
    }
  }
}
console.log(`Archived raw samples, frame-event subsets and evidence to ${destination}`)
