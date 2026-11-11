import test from "node:test"
import assert from "node:assert/strict"
import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

// Domain and platform state is owned by AppServices. Tracker is local to the
// Tauri window adapter; the runtime owner stays separate to control its drop.
const allowed = /^(?:Arc<AppServices>|TokioRuntimeState|Tracker)$/

function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name)
    return entry.isDirectory() ? rustFiles(file) : entry.name.endsWith(".rs") ? [file] : []
  })
}

function violations(source) {
  const errors = []
  const start = /\b(State\s*<|(?:try_state|state)\s*::\s*<)/g
  // Ignore documentation and commented-out examples, preserving line numbers.
  const code = source.replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*/g, (text) => text.replace(/[^\n]/g, " "))
  for (const match of code.matchAll(start)) {
    const open = match.index + match[0].length - 1
    let end = open + 1
    let depth = 1
    while (end < code.length && depth > 0) {
      if (code[end] === "<") depth += 1
      if (code[end] === ">") depth -= 1
      end += 1
    }
    const type = code
      .slice(open + 1, end - 1)
      .replace(/^\s*'\w+\s*,/, "")
      .trim()
    const line = code.slice(0, match.index).split("\n").length
    const normalized = type.replace(/(?:[A-Za-z_]\w*\s*::\s*)+/g, "").replace(/\s/g, "")
    if (!allowed.test(normalized)) errors.push(`${line}: unexpected managed state ${type}`)
  }
  return errors
}

test("managed state scanner allows only the container, runtime and window tracker", () => {
  for (const type of [
    "PtyMap",
    "SftpConnectionPool",
    "SecretStoreState",
    "Arc<PromptBroker>",
    "Arc<SessionLogState>",
    "Arc<TunnelManager>",
    "AppPaths",
    "AppIdentity",
    "Arc<dyn Clipboard>",
    "Arc<dyn MainWindow>",
    "Arc<UnknownState>",
    "AppServices",
  ]) {
    for (const usage of [
      `State<'_, ${type}>`,
      `app.state::<${type}>()`,
      `app.try_state::<\n ${type}\n>()`,
    ]) {
      assert.ok(violations(usage).length > 0, usage)
    }
  }
  for (const type of [
    "Arc<AppServices>",
    "std::sync::Arc<crate::services::AppServices>",
    "TokioRuntimeState",
    "crate::window_state::Tracker",
  ]) {
    assert.deepEqual(violations(`State<'_, ${type}>`), [])
    assert.deepEqual(violations(`app.try_state::<${type}>()`), [])
  }
  assert.deepEqual(violations("// State<'_, TunnelManager>\n/* state::<ZmodemMap>() */"), [])
})

test("Tauri state readers match the remaining managed registrations", () => {
  const root = fileURLToPath(new URL("../../src-tauri/src/", import.meta.url))
  const errors = rustFiles(root).flatMap((file) =>
    violations(fs.readFileSync(file, "utf8")).map(
      (error) => `${path.relative(root, file)}:${error}`
    )
  )
  assert.deepEqual(errors, [])
})
