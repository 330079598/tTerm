// Prints the JavaScript semantics P1-11g has to reproduce or deliberately
// depart from. Run with: node docs/gpui-migration/reports/P1-11g-evidence/semantics.mjs
// Every section names the TypeScript module the value comes from.

const show = (title, value) => console.log(`${title}: ${JSON.stringify(value)}`)
const section = (title) => console.log(`\n## ${title}`)

section("Intl.Collator: sftpView name order vs byte order (sftpView.ts)")
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" })
const names = ["file10.log", "file9.log", "File2.log", "app", "logs", ".env", "Ärger", "apple", "档案", "zebra", "Zebra"]
show("collator", [...names].sort((a, b) => collator.compare(a, b)))
show("bytes", [...names].sort((a, b) => (Buffer.from(a) < Buffer.from(b) ? -1 : 1)))
show("collator 'file9' vs 'file10'", collator.compare("file9.log", "file10.log"))
show("byte     'file9' vs 'file10'", Buffer.compare(Buffer.from("file9.log"), Buffer.from("file10.log")))
show("sensitivity base 'Ärger' vs 'Arger'", collator.compare("Ärger", "Arger"))

section("localeCompare default: profileGroups sibling order (profileGroups.ts)")
const localeNames = ["Web", "api", "Ärger", "zebra", "Zebra", "档案"]
show("locale", [...localeNames].sort((a, b) => a.localeCompare(b)))
show("bytes", [...localeNames].sort((a, b) => (Buffer.from(a) < Buffer.from(b) ? -1 : 1)))
show("'a' vs 'B'", "a".localeCompare("B"))

section("toFixed(1) vs Rust {:.1} (sftpDrawerUtils.formatBytes)")
const sizes = [1024 * 1.25, 1024 * 1.35, 1024 * 2.5, 1024 * 1.005, 1024 * 9.95, 1536, 1280]
for (const value of sizes) {
  const size = value / 1024
  console.log(`  bytes=${value} js=${size.toFixed(1)} exact=${size}`)
}
console.log(`  (Rust format!("{:.1}", 1.25) = "1.2"; js = ${(1.25).toFixed(1)})`)

section("visibleText on astral characters (serial.ts)")
const visible = (text) =>
  Array.from(text, (char) => {
    const code = char.charCodeAt(0)
    if (code >= 0x20 && code !== 0x7f) return char
    return code === 13 ? "\\r" : code === 10 ? "\\n" : code === 9 ? "\\t" : `\\x${code.toString(16).padStart(2, "0")}`
  }).join("")
show("visibleText('a\\u{1F600}b')", visible("a\u{1F600}b"))
show("Array.from length", Array.from("a\u{1F600}b").length)
show("UTF-16 length", "a\u{1F600}b".length)

section("localEchoText split('') on astral characters (serial.ts)")
show("localEchoText('\\u{1F600}\\r')", "\u{1F600}\r".split("\r\n").join("\r").split("").map((c) => (c === "\r" ? "\r\n" : c === "\x7f" || c === "\b" ? "\b \b" : c)).join(""))

section("toLocaleLowerCase and NFKC (settingsSearch.ts, sftpSearch.ts)")
show("'I'.toLocaleLowerCase() default", "I".toLocaleLowerCase())
show("'I'.toLocaleLowerCase('tr')", "I".toLocaleLowerCase("tr"))
show("default locale", Intl.DateTimeFormat().resolvedOptions().locale)
show("'ﬁ' NFKC", "ﬁ".normalize("NFKC"))
show("'①' NFKC", "①".normalize("NFKC"))
show("'Ａ' NFKC", "Ａ".normalize("NFKC"))
show("'İ'.toLowerCase()", "İ".toLowerCase())
show("'İ'.toLowerCase().length", "İ".toLowerCase().length)

section("Number() parsing in validateVariableValues (commandVariables.ts)")
for (const text of ["22", " 22 ", "0x10", "1e3", "Infinity", "2_2", "-0", "007", ""]) {
  console.log(`  Number(${JSON.stringify(text)}) = ${Number(text)} finite=${Number.isFinite(Number(text))}`)
}

section("Date formatting in local time (sftpDrawerUtils.formatTimestamp, serial.timestampPrefix)")
const at = Date.UTC(2026, 0, 1, 9, 5, 7, 42)
show("TZ", process.env.TZ ?? Intl.DateTimeFormat().resolvedOptions().timeZone)
show("offset minutes", new Date(at).getTimezoneOffset())
const date = new Date(at)
show("formatTimestamp components", [date.getFullYear(), date.getMonth() + 1, date.getDate(), date.getHours(), date.getMinutes(), date.getSeconds()])

section("BigUint64 to ms (terminalLogReplay.parseReplayFrames)")
for (const atUs of [1_500n, 1_000_000_000_000n, 9_007_199_254_740_993n, 18_446_744_073_709_551_615n]) {
  console.log(`  atUs=${atUs} atMs=${Number(atUs) / 1000}`)
}

section("glob to regex source (sftpSearch.ts)")
const escapeRegex = (text) => text.replace(/[\\^$+?.()|{}]/g, "\\$&")
function globToRegexSource(glob) {
  let source = "^"
  for (let index = 0; index < glob.length; index += 1) {
    const char = glob[index]
    if (char === "*") { source += ".*"; continue }
    if (char === "?") { source += "."; continue }
    if (char === "[") {
      const closingIndex = glob.indexOf("]", index + 1)
      if (closingIndex === -1) { source += "\\["; continue }
      const classContent = glob.slice(index + 1, closingIndex)
      if (!classContent) { source += "\\[\\]"; index = closingIndex; continue }
      const isNegated = classContent[0] === "!"
      const normalized = isNegated ? classContent.slice(1) : classContent
      source += "[" + (isNegated ? "^" : "") + normalized.replace(/\\/g, "\\\\").replace(/]/g, "\\]") + "]"
      index = closingIndex
      continue
    }
    source += escapeRegex(char)
  }
  return source + "$"
}
for (const glob of ["*.log", "a?c", "[!a]*", "[a-z]1", "x[", "[]", "a.b", "a\\b"]) {
  console.log(`  ${JSON.stringify(glob)} -> ${globToRegexSource(glob)}`)
}
show("i-flag: /^app$/i.test('APP')", /^app$/i.test("APP"))
show("i-flag: /^s$/i.test('ſ')", /^s$/i.test("ſ"))
show("i-flag: /^k$/i.test('K') (Kelvin sign U+212A)", /^k$/i.test("\u212a"))
show("'ß'.toUpperCase()", "ß".toUpperCase())

section("profileSearch name word separators and scoring (profileSearch.ts)")
const NAME_WORD_SEPARATORS = /[\s\-_.]+/
show("'prod-web-01'.split", "prod-web-01".split(NAME_WORD_SEPARATORS))
show("'web.01'.split", "web.01".split(NAME_WORD_SEPARATORS))
