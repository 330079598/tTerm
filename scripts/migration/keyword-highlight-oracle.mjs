// Runs the real TypeScript functions without a browser or changes to src/.
import fs from "node:fs"
import ts from "typescript"

function moduleUrl(path, imports = {}) {
  let source = ts.transpileModule(fs.readFileSync(path, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ESNext },
  }).outputText
  for (const [specifier, url] of Object.entries(imports)) {
    source = source.replaceAll(`"${specifier}"`, JSON.stringify(url))
  }
  return `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
}
const configUrl = moduleUrl("src/lib/keywordHighlight/config.ts")
const colorUrl = moduleUrl("src/components/ThemeEditor/colorUtils.ts")
const config = await import(configUrl)
const matching = await import(moduleUrl("src/lib/keywordHighlight/match.ts", {
  "@/lib/keywordHighlight/config": configUrl,
  "@/components/ThemeEditor/colorUtils": colorUrl,
}))
const palette = { red: "#cc0000", green: "#00cc00", cyan: "#00cccc", magenta: "#cc00cc", yellow: "#cccc00" }
const rule = (pattern) => ({ id: "user", enabled: true, pattern, kind: "regex", case_sensitive: false, whole_word: false, foreground: "red", background: null })
const examples = [
  ["error.", "error中"], ["\\S", "é"], ["[^x]", "é"], ["[\\x80-\\xff]", "é"],
  ["ошибка", "ОШИБКА"], ["\\s", "\u3000"], ["s", "ſ S"], ["k", "K K"],
  [".", "😀"], ["(?=.)|x", "😀x"], ["a{", "a{"], ["]", "]"], ["x{2,1}", "xxx"],
]
const matches = examples.map(([pattern, text]) => {
  const set = { id: "user", name: "User", rules: [rule(pattern)] }
  return { pattern, text, error: matching.highlightPatternError(rule(pattern)), spans: matching.matchHighlights(text, matching.compileHighlightSet(set, palette)).map(({ start, end }) => [start, end]) }
})
const addresses = ["10.0.0.0/33", "Add::add", "x10.0.0.1", "1.2.3.4.5"].map(text => ({ text, matches: matching.matchHighlights(text, matching.compileHighlightSet(config.builtinHighlightSet(), palette)).map(span => text.slice(span.start, span.end)) }))
const colors = ["0 100% 50%", "rgb(1, 2, 3", "rgb(-1,2,3)", "hsl(0, 100, 50)", "hsl(720 100% 50%)"].map(value => ({ value, hex: matching.resolveHighlightColor(value, palette) ?? null }))
console.log(JSON.stringify({ matches, addresses, colors, builtin: config.builtinHighlightSet() }, null, 2))
