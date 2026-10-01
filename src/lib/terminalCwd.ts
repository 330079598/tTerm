// Working directory reports from tTerm's Windows shell integration (see
// src-tauri/src/terminal/shell_integration.rs): OSC 7 `file://host/path` from
// Git Bash and WSL, OSC 9;9 `"path"` from cmd and PowerShell.

/** OSC codes that can carry a working directory report. */
export const CWD_REPORT_OSC_CODES = [7, 9] as const

/**
 * The directory in an OSC 7 or OSC 9;9 payload, or null when the payload is
 * not one. OSC 7 from another host (a shell reached over ssh from this local
 * terminal) is ignored: tTerm's own scripts send an empty host.
 */
export function parseCwdReport(osc: number, data: string): string | null {
  if (osc === 7) return parseOsc7(data)
  if (osc === 9) return parseOsc9(data)
  return null
}

function parseOsc7(data: string): string | null {
  const match = /^file:\/\/([^/]*)(\/.*)$/.exec(data)
  if (!match) return null
  const host = match[1].toLowerCase()
  if (host !== "" && host !== "localhost") return null

  let path = match[2]
  try {
    path = decodeURIComponent(path)
  } catch {
    // A raw, unescaped path from some other emitter; keep it as sent.
  }
  // `file:///C:/Users` names a Windows path.
  if (/^\/[A-Za-z]:([/\\]|$)/.test(path)) path = path.slice(1)
  return path
}

function parseOsc9(data: string): string | null {
  if (!data.startsWith("9;")) return null
  let path = data.slice(2)
  if (path.length >= 2 && path.startsWith('"') && path.endsWith('"')) {
    path = path.slice(1, -1)
  }
  return path.trim() ? path : null
}
