// The reference capture's entry point. `vite --mode parity` serves the real
// React components with the Tauri IPC mocked from parity/fixtures, at the
// window size and theme in parity/screens.json.
//
// It does not mount the application shell: W0-04 only needs the harness that
// W0-05 can grow into a screen per state. See docs/gpui-migration/parity/README.md.

import "@/App.css"
import "@/parity/parity.css"

import { applyUiScalePercent } from "@/lib/uiPlatform"
import { fixtures } from "@/parity/fixtures"
import { parityScreens } from "@/parity/screens"
import { activateFixture, invokedCommands } from "@/parity/tauri/core"
import type { ParityScreenProps } from "@/parity/types"
import { createRoot } from "react-dom/client"
import type { ComponentType } from "react"

const params = new URLSearchParams(window.location.search)
const screenId = params.get("screen") ?? ""
const theme = params.get("theme") ?? "dark"
const scale = Number(params.get("scale") ?? "100")
const platform = params.get("platform") ?? "macos"

window.__PARITY_READY__ = false
window.__PARITY_INVOKES__ = []
window.__PARITY_MISSING__ = []

// `--ui-font-scale` and the `--text-*` variables are normally written by the
// application after it loads the configuration; the capture sets them before
// React mounts so the first paint already has the right type scale.
applyUiScalePercent(Number.isFinite(scale) && scale > 0 ? scale : 100)
if (theme === "light") {
  document.documentElement.dataset.theme = "light"
} else {
  delete document.documentElement.dataset.theme
}
document.documentElement.dataset.platform = platform

let readySent = false

/**
 * Resolves after fonts are ready and two frames have been painted. Screens
 * call it once their fixture-driven state is settled; screens without async
 * data call it from an effect so the first paint is still complete.
 */
function markReady(): void {
  if (readySent) return
  readySent = true
  void document.fonts.ready.then(() => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        window.__PARITY_INVOKES__ = invokedCommands()
        window.__PARITY_READY__ = true
      })
    })
  })
}

const rootElement = document.getElementById("root")
if (!rootElement) {
  throw new Error("Parity entry: #root is missing from parity.html")
}

const Screen = parityScreens[screenId]
const fixture = fixtures[screenId]

if (!screenId || !Screen || !fixture) {
  const known = Object.keys(parityScreens).sort().join(", ")
  const message = !screenId
    ? "Parity entry: the screen query parameter is missing"
    : `Parity entry: no screen or fixture for "${screenId}"; registered: ${known}`
  window.__PARITY_ERROR__ = message
  createRoot(rootElement).render(
    <pre
      style={{ color: "#ff6b6b", fontFamily: "monospace", padding: "16px", whiteSpace: "pre-wrap" }}
    >
      {message}
    </pre>
  )
  markReady()
} else {
  activateFixture(fixture)
  const ScreenComponent: ComponentType<ParityScreenProps> = Screen
  createRoot(rootElement).render(<ScreenComponent onReady={markReady} />)
}
