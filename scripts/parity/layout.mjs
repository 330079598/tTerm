/**
 * Collects the reference layout for one screen.
 *
 * Playwright serialises this function and runs it in the page, so it must not
 * reference anything outside its own body. The shapes match
 * `crates/tterm-parity/src/layout.rs` one for one:
 *
 * - bounds are CSS pixels relative to the viewport, rounded to 0.01;
 * - colours are 8-bit sRGB arrays, read back through a canvas so that any CSS
 *   colour syntax (hsl, rgba, color-mix, color(srgb …)) lands in the same form;
 * - `fontFamily` is canonicalised, because `.SystemUIFont` in GPUI and
 *   `-apple-system` in CSS name the same macOS font;
 * - `textRegions` covers every text node and every input's content box, so the
 *   masked SSIM can leave the text out.
 *
 * `screen` is passed by the caller. `window.__PARITY_INVOKES__` is written by
 * the parity entry point and proves which commands the mocked invoke answered.
 */
export function collectLayout(screen) {
  const round = (value) => Math.round(value * 100) / 100
  const parsePx = (value, property) => {
    const parsed = Number.parseFloat(value)
    if (!Number.isFinite(parsed)) {
      throw new Error(`parity layout: ${property} is "${value}", expected a pixel length`)
    }
    return parsed
  }
  const parseWeight = (value) => {
    if (value === "normal") return 400
    if (value === "bold") return 700
    const parsed = Number.parseInt(value, 10)
    if (!Number.isFinite(parsed)) {
      throw new Error(`parity layout: font-weight is "${value}"`)
    }
    return parsed
  }
  const canonicalFontFamily = (stack) => {
    const first = (stack.split(",")[0] ?? "").trim().replace(/^["']|["']$/g, "").toLowerCase()
    if (
      first === "-apple-system" ||
      first === "blinkmacsystemfont" ||
      first === "system-ui" ||
      first === ".systemuifont" ||
      first === ".applesystemuifont"
    ) {
      return "system-ui"
    }
    return first
  }
  const rect = (value) => ({
    x: round(value.x),
    y: round(value.y),
    width: round(value.width),
    height: round(value.height),
  })

  const canvas = document.createElement("canvas")
  canvas.width = 1
  canvas.height = 1
  const context = canvas.getContext("2d", { willReadFrequently: true })
  if (!context) throw new Error("parity layout: a 2D canvas is unavailable")
  const toRgba = (color) => {
    context.clearRect(0, 0, 1, 1)
    context.fillStyle = color
    context.fillRect(0, 0, 1, 1)
    return Array.from(context.getImageData(0, 0, 1, 1).data)
  }

  const elements = [...document.querySelectorAll("[data-parity]")].map((element) => {
    const style = getComputedStyle(element)
    const id = element.getAttribute("data-parity")
    if (!id) throw new Error("parity layout: an element has an empty data-parity")
    return {
      id,
      sizing: element.getAttribute("data-parity-sizing") === "text" ? "text" : "fixed",
      bounds: rect(element.getBoundingClientRect()),
      style: {
        backgroundColor: toRgba(style.backgroundColor),
        borderColor: toRgba(style.borderTopColor),
        borderWidth: parsePx(style.borderTopWidth, "border-top-width"),
        borderRadius: parsePx(style.borderTopLeftRadius, "border-top-left-radius"),
        fontFamily: canonicalFontFamily(style.fontFamily),
        fontSize: parsePx(style.fontSize, "font-size"),
        fontWeight: parseWeight(style.fontWeight),
        lineHeight: parsePx(style.lineHeight, "line-height"),
        color: toRgba(style.color),
        padding: {
          top: parsePx(style.paddingTop, "padding-top"),
          right: parsePx(style.paddingRight, "padding-right"),
          bottom: parsePx(style.paddingBottom, "padding-bottom"),
          left: parsePx(style.paddingLeft, "padding-left"),
        },
      },
    }
  })

  const textRegions = []
  const seen = new Set()
  const pushRegion = (value) => {
    if (value.width <= 0 || value.height <= 0) return
    const region = rect(value)
    const key = [region.x, region.y, region.width, region.height].join(":")
    if (seen.has(key)) return
    seen.add(key)
    textRegions.push(region)
  }

  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (["SCRIPT", "STYLE", "NOSCRIPT"].includes(parent.tagName)) return NodeFilter.FILTER_REJECT
      return node.textContent && node.textContent.trim()
        ? NodeFilter.FILTER_ACCEPT
        : NodeFilter.FILTER_REJECT
    },
  })
  while (walker.nextNode()) {
    const range = document.createRange()
    range.selectNodeContents(walker.currentNode)
    for (const value of range.getClientRects()) pushRegion(value)
  }

  // An input's value is not a text node, so its whole content box is treated
  // as text and left out of the masked image comparison.
  for (const element of document.querySelectorAll("input, textarea")) {
    const bounds = element.getBoundingClientRect()
    if (bounds.width === 0 || bounds.height === 0) continue
    const style = getComputedStyle(element)
    const left = element.clientLeft + parsePx(style.paddingLeft, "padding-left")
    const top = element.clientTop + parsePx(style.paddingTop, "padding-top")
    const width =
      element.clientWidth -
      parsePx(style.paddingLeft, "padding-left") -
      parsePx(style.paddingRight, "padding-right")
    const height =
      element.clientHeight -
      parsePx(style.paddingTop, "padding-top") -
      parsePx(style.paddingBottom, "padding-bottom")
    pushRegion({ x: bounds.x + left, y: bounds.y + top, width, height })
  }
  textRegions.sort(
    (a, b) => a.y - b.y || a.x - b.x || a.width - b.width || a.height - b.height
  )

  return {
    version: 1,
    screen,
    viewport: {
      width: window.innerWidth,
      height: window.innerHeight,
      deviceScaleFactor: window.devicePixelRatio,
    },
    elements,
    textRegions,
    invokedCommands: [...new Set(window.__PARITY_INVOKES__ ?? [])].sort(),
  }
}
