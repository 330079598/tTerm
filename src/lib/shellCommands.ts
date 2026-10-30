// Splitting the compose bar's text into the commands a shell reads, so that
// running them one at a time sends a command that goes on over several
// lines as one: a line ending in a backslash, an open quote, a
// here-document, or a block such as `if … fi`, `for … done`, `case … esac`
// or a braced one (functions, PowerShell). Sent a line at a time, such a
// command leaves the shell at its continuation prompt, which marks no
// prompt, and the run would wait there for good. Only as much of the shell's
// grammar is read as that takes.

/** Starts a block once `then`, `do` or a brace follows. */
const LEADS = new Set(["if", "elif", "while", "until", "for", "select"])
/** Words after which the next word is a command, so a keyword can follow. */
const PREFIXES = new Set(["then", "do", "else", "elif", "if", "while", "until", "!", "time", "{"])
const BREAKS = /[\s;&|()<>'"`\\]/

interface ScanState {
  quote: "'" | '"' | "`" | null
  /** Open blocks. */
  blocks: number
  /** `if`, `while` and the like still waiting for their `then`, `do` or brace. */
  leads: number
  /** Open parentheses; a case pattern's lone `)` closes none. */
  parens: number
  /** Here-documents whose bodies follow, with the line that ends each. */
  heredocs: { word: string; stripTabs: boolean }[]
}

/** Notes a keyword at the start of a command. */
function noteKeyword(word: string, rest: string, state: ScanState) {
  if (LEADS.has(word)) {
    state.leads += 1
    // `elif` closes the branch `then` opened, and opens one of its own.
    if (word === "elif") state.blocks = Math.max(0, state.blocks - 1)
  } else if ((word === "then" || word === "do") && state.leads > 0) {
    state.leads -= 1
    state.blocks += 1
  } else if (word === "case" && /\sin(\s|$)/.test(rest)) {
    state.blocks += 1
  } else if (word === "fi" || word === "done" || word === "esac") {
    state.blocks = Math.max(0, state.blocks - 1)
  } else if (word === "end" && state.leads > 0) {
    // fish ends its blocks with `end`, without `then` or `do`.
    state.leads -= 1
  }
}

/** Reads one line; true when it ends in a backslash, so the next goes on with it. */
function scanLine(line: string, state: ScanState): boolean {
  let commandStart = true
  let i = 0
  while (i < line.length) {
    const char = line[i]
    if (state.quote) {
      if (char === "\\" && state.quote !== "'") i += 1
      else if (char === state.quote) state.quote = null
      i += 1
    } else if (char === "\\") {
      if (i === line.length - 1) return true
      i += 2
      commandStart = false
    } else if (char === "'" || char === '"' || char === "`") {
      state.quote = char
      commandStart = false
      i += 1
    } else if (char === "#" && (i === 0 || /[\s;&|()]/.test(line[i - 1]))) {
      return false
    } else if (char === ";" || char === "&" || char === "|" || char === "(" || char === ")") {
      if (char === "(") state.parens += 1
      else if (char === ")" && state.parens > 0) state.parens -= 1
      commandStart = true
      i += 1
    } else if (line.startsWith("<<<", i)) {
      // A here-string, on its own line.
      i += 3
    } else if (line.startsWith("<<", i)) {
      const heredoc = /^<<(-?)[ \t]*\\?(['"]?)([^\s'"\\;&|<>()]+)\2/.exec(line.slice(i))
      if (heredoc) state.heredocs.push({ word: heredoc[3], stripTabs: heredoc[1] === "-" })
      i += heredoc ? heredoc[0].length : 2
      commandStart = false
    } else if (/\s/.test(char) || char === "<" || char === ">") {
      i += 1
    } else {
      const start = i
      while (i < line.length && !BREAKS.test(line[i])) i += 1
      const word = line.slice(start, i)
      // Glued to a quote, as in `if"x"`, it is no keyword.
      const whole = i === line.length || !/['"`\\]/.test(line[i])
      if (word === "{") {
        if (state.leads > 0) state.leads -= 1
        state.blocks += 1
      } else if (word === "}") {
        state.blocks = Math.max(0, state.blocks - 1)
      } else if (commandStart && whole) {
        noteKeyword(word, line.slice(i), state)
      }
      commandStart = commandStart && PREFIXES.has(word)
    }
  }
  return false
}

/**
 * The commands in `text`, each a line or the lines that go on with it.
 * Blank lines between commands are dropped, as they run nothing.
 */
export function splitShellCommands(text: string): string[] {
  const state: ScanState = { quote: null, blocks: 0, leads: 0, parens: 0, heredocs: [] }
  const commands: string[] = []
  let current: string[] = []
  for (const line of text.split(/\r\n?|\n/)) {
    if (current.length === 0 && line.trim() === "") continue
    current.push(line)
    let continues = false
    const body = state.heredocs[0]
    if (body) {
      // Opened on an earlier line: this one belongs to the document.
      if ((body.stripTabs ? line.replace(/^\t+/, "") : line) === body.word) state.heredocs.shift()
    } else {
      continues = scanLine(line, state)
    }
    const open =
      continues ||
      state.quote !== null ||
      state.blocks > 0 ||
      state.leads > 0 ||
      state.parens > 0 ||
      state.heredocs.length > 0
    if (!open) {
      commands.push(current.join("\n"))
      current = []
    }
  }
  // A command left open still goes, for the shell to say what it lacks.
  if (current.length > 0) commands.push(current.join("\n"))
  return commands
}
