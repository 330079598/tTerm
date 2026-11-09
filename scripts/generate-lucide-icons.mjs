import { mkdir, readFile, writeFile } from "node:fs/promises"
import { createRequire } from "node:module"

const require = createRequire(import.meta.url)
const packageJson = JSON.parse(await readFile(require.resolve("lucide-react/package.json"), "utf8"))
const names = ["eye", "eye-off", "arrow-big-up-dash", "chevron-down", "x", "triangle-alert", "info"]

function nodeMarkup(node) {
  const [tag, attrs] = node
  return `<${tag} ${Object.entries(attrs).filter(([key]) => key !== "key").map(([key, value]) => `${key}="${value}"`).join(" ")}/>`
}

const icons = {}
for (const name of names) {
  const source = await readFile(require.resolve(`lucide-react/dist/esm/icons/${name}.mjs`), "utf8")
  const match = source.match(/const __iconData = (\{[\s\S]*?\n\});/)
  if (!match) throw new Error(`could not read icon data for ${name}`)
  const nodes = Function(`"use strict"; return (${match[1]})`)().node
  icons[name] = nodes.map(nodeMarkup).join("")
}

await mkdir("crates/tterm-ui/assets/icons", { recursive: true })
for (const [name, body] of Object.entries(icons)) {
  await writeFile(`crates/tterm-ui/assets/icons/${name}.svg`, `<!-- lucide-react ${packageJson.version} -->\n<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>\n`)
}
