// Runs xterm's real CSS color parsing in Chromium and prints the values the
// Rust palette conversion is tested against. The source theme modules are
// untouched.
import fs from "node:fs"
import path from "node:path"
import { chromium } from "playwright"
import ts from "typescript"

const ROOT = path.resolve(import.meta.dirname, "../..")

function moduleUrl(relative) {
  const source = ts.transpileModule(
    fs.readFileSync(path.join(ROOT, relative), "utf8"),
    { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext } }
  ).outputText
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}

const colors = await import(moduleUrl("src/components/ThemeEditor/colorUtils.ts"))

const namedColors = [
  "aliceblue",
  "antiquewhite",
  "aqua",
  "aquamarine",
  "azure",
  "beige",
  "bisque",
  "black",
  "blanchedalmond",
  "blue",
  "blueviolet",
  "brown",
  "burlywood",
  "cadetblue",
  "chartreuse",
  "chocolate",
  "coral",
  "cornflowerblue",
  "cornsilk",
  "crimson",
  "cyan",
  "darkblue",
  "darkcyan",
  "darkgoldenrod",
  "darkgray",
  "darkgreen",
  "darkgrey",
  "darkkhaki",
  "darkmagenta",
  "darkolivegreen",
  "darkorange",
  "darkorchid",
  "darkred",
  "darksalmon",
  "darkseagreen",
  "darkslateblue",
  "darkslategray",
  "darkslategrey",
  "darkturquoise",
  "darkviolet",
  "deeppink",
  "deepskyblue",
  "dimgray",
  "dimgrey",
  "dodgerblue",
  "firebrick",
  "floralwhite",
  "forestgreen",
  "fuchsia",
  "gainsboro",
  "ghostwhite",
  "gold",
  "goldenrod",
  "gray",
  "green",
  "greenyellow",
  "grey",
  "honeydew",
  "hotpink",
  "indianred",
  "indigo",
  "ivory",
  "khaki",
  "lavender",
  "lavenderblush",
  "lawngreen",
  "lemonchiffon",
  "lightblue",
  "lightcoral",
  "lightcyan",
  "lightgoldenrodyellow",
  "lightgray",
  "lightgreen",
  "lightgrey",
  "lightpink",
  "lightsalmon",
  "lightseagreen",
  "lightskyblue",
  "lightslategray",
  "lightslategrey",
  "lightsteelblue",
  "lightyellow",
  "lime",
  "limegreen",
  "linen",
  "magenta",
  "maroon",
  "mediumaquamarine",
  "mediumblue",
  "mediumorchid",
  "mediumpurple",
  "mediumseagreen",
  "mediumslateblue",
  "mediumspringgreen",
  "mediumturquoise",
  "mediumvioletred",
  "midnightblue",
  "mintcream",
  "mistyrose",
  "moccasin",
  "navajowhite",
  "navy",
  "oldlace",
  "olive",
  "olivedrab",
  "orange",
  "orangered",
  "orchid",
  "palegoldenrod",
  "palegreen",
  "paleturquoise",
  "palevioletred",
  "papayawhip",
  "peachpuff",
  "peru",
  "pink",
  "plum",
  "powderblue",
  "purple",
  "rebeccapurple",
  "red",
  "rosybrown",
  "royalblue",
  "saddlebrown",
  "salmon",
  "sandybrown",
  "seagreen",
  "seashell",
  "sienna",
  "silver",
  "skyblue",
  "slateblue",
  "slategray",
  "slategrey",
  "snow",
  "springgreen",
  "steelblue",
  "tan",
  "teal",
  "thistle",
  "tomato",
  "turquoise",
  "violet",
  "wheat",
  "white",
  "whitesmoke",
  "yellow",
  "yellowgreen",
]

const values = [
  ...namedColors,
  "#abc",
  "#AABBCC",
  "#1b1d2300",
  "#abcd",
  "#AABBCCDD",
  "rgb(1, 2, 3)",
  "rgba(1, 2, 3, 0.5)",
  "rgba(1, 2, 3, 1)",
  "hsl(207 100% 40%)",
  "hsl(207 100% 40% / 0.24)",
  "hsl(207 100% 40% / 24%)",
  "hsla(207, 100%, 40%, 0.24)",
  "transparent",
  "notacolor",
]

const roles = [
  "background",
  "foreground",
  "cursor",
  "cursorAccent",
  "selectionBackground",
  "selectionForeground",
  "black",
  "brightWhite",
]

const roleValues = [
  "#abc",
  "#AABBCC",
  "#1b1d2300",
  "#abcd",
  "#AABBCCDD",
  "rgb(1, 2, 3)",
  "rgba(1, 2, 3, 0.5)",
  "hsl(207 100% 40%)",
  "hsl(207 100% 40% / 0.24)",
  "hsl(207 100% 40% / 24%)",
  "hsla(207, 100%, 40%, 0.24)",
  "tomato",
  "transparent",
  "notacolor",
]

const browser = await chromium.launch({ headless: true })
const page = await browser.newPage()
await page.setContent('<div id="host" style="width:400px;height:200px"></div>')
await page.addScriptTag({
  path: new URL("../../node_modules/@xterm/xterm/lib/xterm.js", import.meta.url).pathname,
})

const result = await page.evaluate(({ values, roles, roleValues }) => {
  const canvas = document.createElement("canvas")
  canvas.width = 1
  canvas.height = 1
  const context = canvas.getContext("2d", { willReadFrequently: true })
  context.globalCompositeOperation = "copy"
  const empty = context.createLinearGradient(0, 0, 1, 1)

  const css = values.map((value) => {
    context.fillStyle = empty
    context.fillStyle = value
    if (typeof context.fillStyle !== "string") return { value, rgba: null }
    context.fillRect(0, 0, 1, 1)
    const [red, green, blue, alpha] = context.getImageData(0, 0, 1, 1).data
    return { value, rgba: [red, green, blue, alpha] }
  })

  const xterm = values.map((value) => {
    const host = document.createElement("div")
    host.style.width = "200px"
    host.style.height = "100px"
    document.body.appendChild(host)
    const terminal = new Terminal({ theme: { background: value } })
    terminal.open(host)
    const color = terminal._core._themeService._colors.background
    const rgba = [
      (color.rgba >>> 24) & 0xff,
      (color.rgba >>> 16) & 0xff,
      (color.rgba >>> 8) & 0xff,
      color.rgba & 0xff,
    ]
    terminal.dispose()
    host.remove()
    return { value, css: color.css, rgba }
  })

  const baseline = {
    background: "#101010",
    foreground: "#eeeeee",
    cursor: "#eeeeee",
    cursorAccent: "#101010",
    selectionBackground: "rgba(1, 2, 3, 0.5)",
    selectionForeground: "#eeeeee",
    black: "#111111",
    brightWhite: "#ffffff",
  }
  const read = (color) => {
    if (!color) return null
    return [
      (color.rgba >>> 24) & 0xff,
      (color.rgba >>> 16) & 0xff,
      (color.rgba >>> 8) & 0xff,
      color.rgba & 0xff,
    ]
  }
  const fields = roles.flatMap((role) =>
    roleValues.map((value) => {
      const host = document.createElement("div")
      host.style.width = "200px"
      host.style.height = "100px"
      document.body.appendChild(host)
      const terminal = new Terminal({ theme: { ...baseline, [role]: value } })
      terminal.open(host)
      const colors = terminal._core._themeService._colors
      const color =
        role === "selectionBackground"
          ? colors.selectionBackgroundTransparent
          : role === "black"
            ? colors.ansi[0]
            : role === "brightWhite"
              ? colors.ansi[15]
              : colors[role]
      terminal.dispose()
      host.remove()
      return { role, value, rgba: read(color) }
    })
  )

  return { css, xterm, fields }
}, { values, roles, roleValues })

await browser.close()

const themePalette = [
  "hsl(207 100% 40%)",
  "hsl(207 100% 40% / 0.24)",
  "hsl(207 100% 40% / 24%)",
  "hsla(207, 100%, 40%, 0.24)",
].map((value) => {
  const xterm = result.xterm.find((candidate) => candidate.value === value)
  return { value, hex: colors.colorToHex(value), rgba: xterm.rgba }
})

process.stdout.write(`${JSON.stringify({ ...result, themePalette }, null, 2)}\n`)
