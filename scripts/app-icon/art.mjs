// Draws tTerm's app icon, a hermit crab whose shell is a terminal window,
// as SVG. Both drawings sit on a 1024 x 1024 tile. The full one is wrapped
// in a sticker filter that roughens the ink lines and adds the white border
// and drop shadow. The filter needs a Chromium-class renderer: resvg, which
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

/** The full drawing, used above 48 px. */
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
 * The drawing for 48 px and below, which is what the Windows taskbar, title
 * bar and Explorer show. There the full drawing's subject covers half the
 * tile and its sticker border and rough lines blur into grey, so this one
 * drops the sun, sand, sticker filter, legs and face, keeps the lines
 * straight so they land on pixels, and lets the window and crab fill the
 * tile, with the eyes over the dark screen where they stand out most.
 */
export function smallIcon() {
  const window = `
    <rect x="250" y="50" width="730" height="580" rx="96" fill="${SCREEN}" stroke="none"/>
    <path d="M346 50 H884 A96 96 0 0 1 980 146 V210 H250 V146 A96 96 0 0 1 346 50 Z" fill="${AMBER}" stroke="none"/>
    <rect x="250" y="50" width="730" height="580" rx="96" fill="none"/>
    <path d="M250 210 H980"/>
    <path d="M650 300 L760 380 L650 460 M800 470 H900" fill="none" stroke="${AMBER}" stroke-width="56"/>`
  // The far claw comes first so the body covers its arm.
  const crab = `
    <path d="M880 840 H800" stroke-width="50"/>
    <path d="M880 840 L960 790 A92 92 0 1 0 966 880 Z" fill="${CORAL}"/>
    <path d="M330 640 L320 470 M540 640 L550 470" stroke-width="48"/>
    <ellipse cx="480" cy="790" rx="370" ry="190" fill="${CORAL}"/>
    <circle cx="320" cy="420" r="96" fill="#ffffff"/>
    <circle cx="345" cy="440" r="44" fill="${INK}" stroke="none"/>
    <circle cx="550" cy="420" r="96" fill="#ffffff"/>
    <circle cx="575" cy="440" r="44" fill="${INK}" stroke="none"/>
    <path d="M150 740 L270 790" stroke-width="60"/>
    <path d="M150 740 L52 594 A140 140 0 1 0 192 568 Z" fill="${CORAL}"/>`
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#ffd88a"/><stop offset="1" stop-color="#f0905a"/>
    </linearGradient>
  </defs>
  <rect width="1024" height="1024" rx="200" fill="url(#bg)"/>
  <g transform="translate(512 512) scale(0.86) translate(-505 -520)" stroke="${INK}" stroke-width="44" stroke-linecap="round" stroke-linejoin="round">${window}${crab}
  </g>
</svg>
`
}
