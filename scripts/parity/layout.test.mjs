import assert from "node:assert/strict"
import { test } from "node:test"
import { collectLayout } from "./layout.mjs"

test("computed shadows select visible zero-offset zero-blur outer spread", (t) => {
  const shadows = [
    ["none", 0],
    ["rgba(0, 112, 204, 0.5) 0px 0px 0px 3px", 3],
    ["0px 0px 3px 5px rgb(0, 112, 204)", 0],
    ["rgb(0, 112, 204) 1px 0px 0px 5px", 0],
    ["inset 0px 0px 0px 5px rgb(0, 112, 204)", 0],
    ["rgba(0, 0, 0, 0) 0px 0px 0px 8px, rgb(0, 112, 204) 0px 0px 0px 3px, rgb(0, 0, 0) 0px 1px 2px 9px", 3],
    ["0px 0px 0px 2px color(srgb 0 0.4 0.8 / 0.5), 0px 0px 0px 3px rgb(0, 112, 204)", 3],
    ["rgb(0, 112, 204) 0px 0px 0px -1px", 0],
  ]
  const elements = shadows.map(([boxShadow], index) => ({
    nodeType: 1,
    parentElement: null,
    boxShadow,
    getAttribute: (name) => name === "data-parity" ? `shadow-${index}` : null,
    getBoundingClientRect: () => ({ x: 10, y: 10, width: 100, height: 36 }),
  }))
  const context = {
    fillStyle: "",
    clearRect() {},
    fillRect() {},
    getImageData() {
      return { data: [0, 112, 204, this.fillStyle === "rgba(0, 0, 0, 0)" ? 0 : 255] }
    },
  }
  const globals = {
    window: { location: { search: "" }, innerWidth: 800, innerHeight: 600, devicePixelRatio: 2 },
    Node: { ELEMENT_NODE: 1 },
    NodeFilter: { SHOW_TEXT: 4 },
    document: {
      body: {},
      createElement: () => ({ getContext: () => context }),
      querySelectorAll: (selector) => selector === "[data-parity]" ? elements : [],
      createTreeWalker: () => ({ nextNode: () => false }),
    },
    getComputedStyle: (element) => new Proxy({
      boxShadow: element.boxShadow,
      opacity: "1",
      fontFamily: "system-ui",
      fontWeight: "500",
      backgroundColor: "rgb(0, 112, 204)",
      borderTopColor: "rgb(0, 112, 204)",
      color: "rgb(0, 112, 204)",
    }, { get: (target, key) => target[key] ?? "0px" }),
  }
  for (const [name, value] of Object.entries(globals)) {
    const previous = Object.getOwnPropertyDescriptor(globalThis, name)
    Object.defineProperty(globalThis, name, { value, configurable: true })
    t.after(() => previous ? Object.defineProperty(globalThis, name, previous) : delete globalThis[name])
  }
  const layout = collectLayout("shadows")
  assert.equal(layout.version, 3)
  assert.deepEqual(layout.elements.map((element) => element.style.ringSpread), shadows.map(([, spread]) => spread))
})
