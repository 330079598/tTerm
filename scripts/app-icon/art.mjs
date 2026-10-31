// Draws tTerm's app icon, a hermit crab whose shell is a terminal window,
// as SVG. Both drawings sit on a 1024 x 1024 beach tile and are wrapped in a
// sticker filter that roughens the ink lines and adds the white border and
// drop shadow. The filter needs a Chromium-class renderer: resvg, which
// `tauri icon` uses for SVG input, smears the displacement into blobs.

const INK = "#1f1a33"
const AMBER = "#ffc23d"
const CORAL = "#ff7059"
const CREAM = "#fff4dc"
const SCREEN = "#2b2752"

const STICKER_FILTER = `
    <filter id="sticker" x="-20%" y="-20%" width="140%" height="140%" color-interpolation-filters="sRGB">
      <feTurbulence type="fractalNoise" baseFrequency="0.018" numOctaves="2" seed="3" result="noise"/>
      <feDisplacementMap in="SourceGraphic" in2="noise" scale="12" xChannelSelector="R" yChannelSelector="G" result="rough"/>
      <feMorphology in="rough" operator="dilate" radius="22" result="fat"/>
      <feFlood flood-color="#ffffff"/>
      <feComposite in2="fat" operator="in" result="border"/>
      <feOffset in="border" dy="16" result="drop"/>
      <feFlood flood-color="#0b0820" flood-opacity="0.35"/>
      <feComposite in2="drop" operator="in"/>
      <feGaussianBlur stdDeviation="10" result="shadow"/>
      <feMerge><feMergeNode in="shadow"/><feMergeNode in="border"/><feMergeNode in="rough"/></feMerge>
    </filter>`

/** The beach tile: a warm sky, a pale sun and a strip of sand. */
const BACKGROUND = `
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#ffd88a"/><stop offset="1" stop-color="#f0905a"/>
    </linearGradient>${STICKER_FILTER}
  </defs>
  <clipPath id="tile"><rect width="1024" height="1024" rx="228"/></clipPath>
  <g clip-path="url(#tile)">
    <rect width="1024" height="1024" fill="url(#bg)"/>
    <circle cx="830" cy="190" r="72" fill="#fff6d6" opacity="0.85"/>
    <path d="M0 860 Q260 800 512 850 T1024 830 V1024 H0 Z" fill="#f3d39b" opacity="0.9"/>
  </g>`

/** A `>_` prompt whose chevron is `size` wide, drawn from its top-left corner. */
function prompt(x, y, size, width) {
  return `
      <path d="M${x} ${y} L${x + size} ${y + size * 0.7} L${x} ${y + size * 1.4}" fill="none" stroke="${AMBER}" stroke-width="${width}"/>
      <path d="M${x + size * 1.5} ${y + size * 1.45} H${x + size * 2.7}" stroke="${AMBER}" stroke-width="${width}"/>`
}

function sticker(art, transform) {
  const placement = transform ? ` transform="${transform}"` : ""
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">${BACKGROUND}
  <g filter="url(#sticker)"${placement} stroke="${INK}" stroke-width="18" stroke-linecap="round" stroke-linejoin="round">${art}
  </g>
</svg>
`
}

function eye(cx, cy) {
  return `
    <circle cx="${cx}" cy="${cy}" r="50" fill="#ffffff"/>
    <circle cx="${cx + 13}" cy="${cy + 10}" r="21" fill="${INK}" stroke="none"/>
    <circle cx="${cx + 5}" cy="${cy}" r="7" fill="#ffffff" stroke="none"/>`
}

/** The full drawing, used from 48 px up. */
export function fullIcon() {
  const shell = `
    <g transform="rotate(-8 615 450)">
      <rect x="400" y="250" width="430" height="390" rx="50" fill="${CREAM}"/>
      <path d="M450 250 H780 A50 50 0 0 1 830 300 V340 H400 V300 A50 50 0 0 1 450 250 Z" fill="${AMBER}" stroke="none"/>
      <rect x="400" y="250" width="430" height="390" rx="50" fill="none"/>
      <path d="M400 340 H830"/>
      <circle cx="455" cy="295" r="13" fill="${INK}" stroke="none"/>
      <circle cx="497" cy="295" r="13" fill="${INK}" stroke="none"/>
      <circle cx="539" cy="295" r="13" fill="${INK}" stroke="none"/>
      <rect x="440" y="375" width="350" height="225" rx="24" fill="${SCREEN}" stroke-width="12"/>${prompt(530, 430, 58, 26)}
    </g>`
  // Eye stalks and legs come before the body so the body covers their roots.
  const crab = `
    <path d="M345 625 Q330 545 322 480" fill="none" stroke-width="22"/>
    <path d="M440 620 Q455 545 470 480" fill="none" stroke-width="22"/>
    <path d="M300 785 Q270 830 250 865" fill="none" stroke-width="20"/>
    <path d="M375 800 Q370 840 360 880" fill="none" stroke-width="20"/>
    <path d="M455 795 Q475 835 495 870" fill="none" stroke-width="20"/>
    <ellipse cx="395" cy="700" rx="175" ry="112" fill="${CORAL}"/>
    <path d="M360 708 Q395 738 430 708" fill="none" stroke-width="12"/>
    <ellipse cx="318" cy="700" rx="26" ry="14" fill="#ffb3a6" stroke="none"/>
    <ellipse cx="472" cy="700" rx="26" ry="14" fill="#ffb3a6" stroke="none"/>${eye(320, 455)}${eye(472, 455)}
    <path d="M262 690 C268 605 165 582 112 640 L182 680 L108 714 C140 790 258 778 262 690 Z" fill="${CORAL}"/>
    <path d="M545 765 C540 718 600 702 632 732 L595 756 L637 778 C615 820 550 812 545 765 Z" fill="${CORAL}"/>`
  return sticker(shell + crab, "translate(40 -30)")
}

/**
 * The drawing for 16 to 32 px. Legs, blush, mouth and window buttons vanish
 * at that size, so it keeps only a bigger shell, the eyes and one claw, with
 * heavier lines.
 */
export function smallIcon() {
  const art = `
    <g transform="rotate(-8 600 440)" stroke-width="28">
      <rect x="360" y="200" width="500" height="460" rx="64" fill="${CREAM}"/>
      <path d="M424 200 H796 A64 64 0 0 1 860 264 V320 H360 V264 A64 64 0 0 1 424 200 Z" fill="${AMBER}" stroke="none"/>
      <rect x="360" y="200" width="500" height="460" rx="64" fill="none"/>
      <path d="M360 320 H860"/>
      <rect x="410" y="370" width="400" height="240" rx="28" fill="${SCREEN}" stroke-width="16"/>${prompt(500, 418, 70, 38)}
    </g>
    <path d="M335 640 Q320 560 312 500 M450 635 Q465 560 478 500" fill="none" stroke-width="30"/>
    <ellipse cx="390" cy="720" rx="210" ry="130" fill="${CORAL}" stroke-width="26"/>
    <circle cx="310" cy="470" r="64" fill="#ffffff" stroke-width="24"/>
    <circle cx="326" cy="482" r="28" fill="${INK}" stroke="none"/>
    <circle cx="478" cy="470" r="64" fill="#ffffff" stroke-width="24"/>
    <circle cx="494" cy="482" r="28" fill="${INK}" stroke="none"/>
    <path d="M250 715 C256 615 135 590 80 655 L160 700 L76 738 C112 830 246 818 250 715 Z" fill="${CORAL}" stroke-width="26"/>`
  // The scale goes inside the filtered group: on the group itself it would
  // also shrink the border, shadow and roughness, which are in user units.
  return sticker(`
    <g transform="translate(512 512) scale(0.88) translate(-470 -520)">${art}
    </g>`)
}
