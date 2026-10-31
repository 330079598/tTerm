// Draws the crab that moves: the mascot seen from the front, a terminal window
// for a body with two eyes on top and a claw on each side. Each part that
// moves is its own SVG, so the page can stack them and animate each as an
// HTML box, which every webview hands to the compositor; an animation inside
// an SVG repaints it each frame in WebKit. The parts share one viewBox, so
// stacked at the same size they line up. Only the left claw is drawn: the
// right one is the left mirrored across the middle.
//
// The full crab is for the splash screen and dialogs. The small one, for a
// tab badge or a spinner, keeps to an 18 px grid: the claws hug the body, the
// eyes are bigger, and the screen keeps only the chevron.

import { mkdirSync, writeFileSync } from "node:fs"
import { join } from "node:path"

const INK = "#1f1a33"
const AMBER = "#ffc23d"
const CORAL = "#ff7059"
const CREAM = "#fff4dc"
const SCREEN = "#2b2752"

const FULL_VIEWBOX = "0 24 240 176"
const SMALL_VIEWBOX = "0 0 28 18"

function svg(viewBox, body) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}" fill="${CORAL}" stroke="${INK}" stroke-linejoin="round">${body}
</svg>
`
}

const round = (n) => +n.toFixed(1)

/**
 * One jaw of a claw. A claw is a disc with a notch `notch` degrees either side
 * of straight up, split down the middle into two jaws that hinge at the bottom
 * of the disc. Only the rim and the notch are outlined, so the split does not
 * show and the jaws can overlap as they close.
 */
function jaw(cx, cy, r, notch, outer) {
  const angle = ((-90 + (outer ? -notch : notch)) * Math.PI) / 180
  const tip = `${round(cx + r * Math.cos(angle))} ${round(cy + r * Math.sin(angle))}`
  // The notch runs a little past the centre, so the jaws meet in a V.
  const rim = `M${cx} ${cy + r} A${r} ${r} 0 0 ${outer ? 1 : 0} ${tip} L${cx} ${round(cy + r * 0.17)}`
  return `
  <path d="${rim} L${cx} ${cy + r} Z" stroke="none"/>
  <path d="${rim}" fill="none"/>`
}

/** An arm: an ink line under a narrower coral one. */
function arm(x1, x2, y, width, core) {
  return `
  <path d="M${x1} ${y} L${x2} ${y}" stroke-width="${width}" stroke-linecap="round"/>
  <path d="M${x1} ${y} L${x2} ${y}" stroke="${CORAL}" stroke-width="${core}" stroke-linecap="round"/>`
}

/** Three short strokes fanning out under a claw as it lands. */
function hit(points, width) {
  const strokes = points.map(([x1, y1, x2, y2]) => `<path d="M${x1} ${y1} L${x2} ${y2}"/>`)
  return `
  <g stroke="${AMBER}" stroke-width="${width}" stroke-linecap="round">${strokes.join("")}</g>`
}

function eye(cx, cy, r, ring, pupil, glint) {
  return `
  <circle cx="${cx}" cy="${cy}" r="${r}" fill="#ffffff" stroke-width="${ring}"/>
  <circle cx="${cx + r / 6}" cy="${cy + r / 7}" r="${pupil}" fill="${INK}" stroke="none"/>${
    glint
      ? `\n  <circle cx="${cx + r / 3.3}" cy="${cy - 1}" r="${glint}" fill="#ffffff" stroke="none"/>`
      : ""
  }`
}

/** The window the crab wears as a body, its title bar amber and its screen showing `screen`. */
function windowBody(x, y, w, h, r, bar, outline, screen) {
  return `
  <rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${r}" fill="${CREAM}" stroke="none"/>
  <path d="M${x + r} ${y} H${x + w - r} A${r} ${r} 0 0 1 ${x + w} ${y + r} V${y + bar} H${x} V${y + r} A${r} ${r} 0 0 1 ${x + r} ${y} Z" fill="${AMBER}" stroke="none"/>
  <rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${r}" fill="none" stroke-width="${outline}"/>${screen}`
}

const FULL_CLAW = { cx: 40, cy: 120, r: 23, notch: 30 }

/** The parts of the full crab, by file name. */
export function fullCrab() {
  const { cx, cy, r, notch } = FULL_CLAW
  const screen = `
  <rect x="82" y="106" width="76" height="48" rx="9" fill="${SCREEN}" stroke-width="4"/>
  <path d="M94 117 L108 130 L94 143" fill="none" stroke="${AMBER}" stroke-width="9" stroke-linecap="round"/>
  <rect x="114" y="136" width="28" height="9" rx="4.5" fill="${AMBER}" stroke="none"/>`
  return {
    "body.svg": svg(FULL_VIEWBOX, windowBody(70, 68, 100, 98, 20, 28, 8, screen)),
    "eyes.svg": svg(FULL_VIEWBOX, eye(100, 56, 15, 6, 6.5, 2) + eye(140, 56, 15, 6, 6.5, 2)),
    "arm.svg": svg(FULL_VIEWBOX, arm(76, 44, 142, 14, 8)),
    "jaw-outer.svg": svg(
      FULL_VIEWBOX,
      `<g stroke-width="6">${jaw(cx, cy, r, notch, true)}\n  </g>`
    ),
    "jaw-inner.svg": svg(
      FULL_VIEWBOX,
      `<g stroke-width="6">${jaw(cx, cy, r, notch, false)}\n  </g>`
    ),
    "hit.svg": svg(
      FULL_VIEWBOX,
      hit(
        [
          [20, 160, 10, 167],
          [40, 166, 40, 178],
          [60, 160, 70, 167],
        ],
        5
      )
    ),
  }
}

/** The parts of the small crab, by file name. */
export function smallCrab() {
  const screen = `
  <rect x="9.9" y="9.2" width="8.2" height="5.9" rx="1" fill="${SCREEN}" stroke="none"/>
  <path d="M11.6 10.4 L13.6 12.15 L11.6 13.9" fill="none" stroke="${AMBER}" stroke-width="1.3" stroke-linecap="round"/>`
  const claw = `<g stroke-width="1.3">${jaw(4.2, 9.6, 3.6, 32, true)}${jaw(4.2, 9.6, 3.6, 32, false)}\n  </g>`
  return {
    "small-body.svg": svg(
      SMALL_VIEWBOX,
      windowBody(8.2, 4.2, 11.6, 12.6, 2.6, 3.6, 1.6, screen) +
        eye(11.4, 3.4, 2.5, 1.2, 1.45, 0) +
        eye(16.6, 3.4, 2.5, 1.2, 1.45, 0)
    ),
    "small-claw.svg": svg(SMALL_VIEWBOX, arm(9, 5, 12.5, 3.2, 1.2) + claw),
    "small-hit.svg": svg(
      SMALL_VIEWBOX,
      hit(
        [
          [1, 15.6, 0.2, 17],
          [4.2, 16.2, 4.2, 17.6],
          [7.4, 15.6, 8.2, 17],
        ],
        1.2
      )
    ),
  }
}

/** Writes every part of both crabs into `dir`. */
export function writeCrabParts(dir) {
  mkdirSync(dir, { recursive: true })
  for (const [name, content] of Object.entries({ ...fullCrab(), ...smallCrab() })) {
    writeFileSync(join(dir, name), content)
  }
}
