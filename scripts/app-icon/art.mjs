// Draws tTerm's logo and mascot as 1024 x 1024 SVG. The logo, a prompt
// chevron beside a crab on the mascot's beach, is the app icon at every
// size. The mascot, a hermit crab whose shell is a terminal window, is the
// large picture in the README, on the splash screen and in the About dialog,
// where its detail has room. The mascot is wrapped in a sticker filter that
// roughens the ink lines and adds the white border and drop shadow. The
// filter needs a Chromium-class renderer: resvg, which `tauri icon` uses for
// SVG input, smears the displacement into blobs.

const INK = "#1f1a33"
const AMBER = "#ffc23d"
const CORAL = "#ff7059"
// Deeper than CORAL so the logo's crab stands out from the orange sky.
const CRAB_RED = "#e8453c"
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

/** The pale sun and the strip of sand that the mascot and the logo share. */
const SUN_AND_SAND = `
    <circle cx="830" cy="190" r="72" fill="#fff6d6" opacity="0.85"/>
    <path d="M0 860 Q260 800 512 850 T1024 830 V1024 H0 Z" fill="#f3d39b" opacity="0.9"/>`

/** The beach tile: a warm sky, a pale sun and a strip of sand. */
const BACKGROUND = `
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#ffd88a"/><stop offset="1" stop-color="#f0905a"/>
    </linearGradient>${STICKER_FILTER}
  </defs>
  <clipPath id="tile"><rect width="1024" height="1024" rx="228"/></clipPath>
  <g clip-path="url(#tile)">
    <rect width="1024" height="1024" fill="url(#bg)"/>${SUN_AND_SAND}
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

/** The mascot on its beach tile. */
export function mascotIcon() {
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

/** A crab claw: a disc with a wedge cut out, its mouth pointing `dir` degrees. */
function claw(cx, cy, r, dir) {
  const point = (degrees) => {
    const angle = (degrees * Math.PI) / 180
    return `${(cx + r * Math.cos(angle)).toFixed(1)} ${(cy + r * Math.sin(angle)).toFixed(1)}`
  }
  return `M${cx} ${cy} L${point(dir + 30)} A${r} ${r} 0 1 1 ${point(dir - 30)} Z`
}

/**
 * The logo: a `>` prompt beside a crab, as two solid shapes in two colours,
 * the way the icons that stay readable on a taskbar are drawn. A drawing with
 * more parts turned to texture at 24 px. The claws sit close to the body so
 * the pair is narrow enough to be drawn large on the tile. The pupils are filled with the sky,
 * so they read as holes. Each claw is its own group, pivoting on its centre,
 * so a page can animate it.
 */
const LOGO = `
  <g transform="translate(512 540) scale(1.12) translate(-512 -540)">
  <path d="M160 357 L296 517 L160 677" fill="none" stroke="${INK}" stroke-width="104" stroke-linecap="round" stroke-linejoin="round"/>
  <g fill="${CRAB_RED}" stroke="${CRAB_RED}" stroke-linecap="round">
    <g id="claw-left"><path d="M506 640 L482 600" stroke-width="60"/><path d="${claw(482, 578, 74, -100)}" stroke="none"/></g>
    <g id="claw-right"><path d="M816 640 L840 600" stroke-width="60"/><path d="${claw(840, 578, 74, -80)}" stroke="none"/></g>
    <path d="M591 580 V428 M731 580 V428" stroke-width="52"/>
    <rect x="486" y="560" width="350" height="216" rx="108" stroke="none"/>
    <circle cx="591" cy="428" r="70" stroke="none"/>
    <circle cx="731" cy="428" r="70" stroke="none"/>
  </g>
  <circle cx="601" cy="438" r="29" fill="url(#sky)"/>
  <circle cx="741" cy="438" r="29" fill="url(#sky)"/>
  </g>`

/**
 * The logo on the mascot's beach, on a rounded tile `inset` px in from each
 * edge of the 1024 px canvas. The tile is drawn with a 24 px inset and
 * scaled to fit.
 */
function tiled(inset) {
  const scale = ((1024 - 2 * inset) / 976).toFixed(4)
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <defs>
    <linearGradient id="sky" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0" y2="1024">
      <stop offset="0" stop-color="#ffd88a"/><stop offset="1" stop-color="#f0905a"/>
    </linearGradient>
    <clipPath id="tile"><rect x="24" y="24" width="976" height="976" rx="220"/></clipPath>
  </defs>
  <g transform="translate(512 512) scale(${scale}) translate(-512 -512)">
    <g clip-path="url(#tile)">
      <rect width="1024" height="1024" fill="url(#sky)"/>${SUN_AND_SAND}
    </g>${LOGO}
  </g>
</svg>
`
}

/** The app icon for Windows and Linux, its tile reaching the canvas edges the way Windows icons do. */
export function appIcon() {
  return tiled(0)
}

/** The app icon for macOS, whose template leaves a 100 px margin around the tile. */
export function macIcon() {
  return tiled(100)
}
