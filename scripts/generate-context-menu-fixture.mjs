// Keep the native parity menu's data and default chords derived from production TS.
import { createServer } from "vite"
import { writeFile } from "node:fs/promises"
const server = await createServer({ mode: "parity", server: { middlewareMode: true } })
try {
  const { contextMenuFixture } = await server.ssrLoadModule("/src/parity/screens/contextMenuFixture.ts")
  await writeFile("parity/fixtures/ui.context-menu.json", JSON.stringify(contextMenuFixture(), null, 2) + "\n")
} finally {
  await server.close()
}
