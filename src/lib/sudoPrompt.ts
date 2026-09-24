import type { IBuffer } from "@xterm/xterm"

export interface PasswordPromptMatch {
  /** The prompt as displayed, trimmed; the backend checks it is still waiting. */
  prompt: string
  /** The account whose password is requested, or null when the prompt does not say. */
  user: string | null
}

/** Built-in prompts whose first capture group is the account name. */
const NAMED_USER_PROMPTS: RegExp[] = [
  // sudo, C/English locale; newer releases quote the name.
  /^\[sudo\] password for '?(\S+?)'?:$/,
  // sudo, zh_CN and zh_TW locales (full-width colon).
  /^\[sudo\] (\S+) 的密[码碼][:：]$/,
  // OpenBSD doas.
  /^doas \((\S+?)@[^)]*\) password:$/,
]

/** Any other sudo locale: `[sudo] ` prefix, the user somewhere, colon last. */
const LOCALIZED_SUDO_PROMPT = /^\[sudo\] .+[:：]$/

/**
 * sudo-rs (Ubuntu 25.10+) prints `[sudo: authenticate] <PAM prompt>:` without
 * the account; PAM may also ask for a PIN or OTP there, so only password
 * prompts qualify.
 */
const SUDO_RS_PROMPT = /^\[sudo: authenticate\] (.+)[:：]$/
const PAM_PASSWORD_WORD =
  /password|passwort|passwd|mot de passe|contraseña|senha|пароль|密码|密碼|パスワード|비밀번호|암호/i

/** A long line cannot be a password prompt; skip matching it. */
const MAX_PROMPT_CHARS = 256

/**
 * Compiles user-configured prompt patterns, dropping invalid ones. A named
 * group `user` identifies the account; without it the prompt is matched for
 * any user.
 */
export function compilePromptPatterns(patterns: readonly string[]): RegExp[] {
  const compiled: RegExp[] = []
  for (const pattern of patterns) {
    const source = pattern.trim()
    if (!source) continue
    try {
      compiled.push(new RegExp(source))
    } catch {
      // Settings validate patterns on input; skip anything that slipped through.
    }
  }
  return compiled
}

export function isValidPromptPattern(pattern: string): boolean {
  try {
    new RegExp(pattern)
    return true
  } catch {
    return false
  }
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
}

/**
 * Matches a terminal line against the built-in and custom password prompts.
 * `knownUser` is the session's login name, used to recognize localized sudo
 * prompts whose wording is not known ahead of time.
 */
export function matchPasswordPrompt(
  line: string,
  knownUser: string | undefined,
  customPatterns: readonly RegExp[] = []
): PasswordPromptMatch | null {
  const prompt = line.trim()
  if (!prompt || prompt.length > MAX_PROMPT_CHARS) return null
  // Every accepted prompt ends in a colon; the backend enforces the same rule.
  if (!prompt.endsWith(":") && !prompt.endsWith("：")) return null

  for (const pattern of NAMED_USER_PROMPTS) {
    const match = prompt.match(pattern)
    if (match) return { prompt, user: match[1] }
  }

  // sudo-rs asks for the invoking user's password but does not name them.
  const sudoRs = prompt.match(SUDO_RS_PROMPT)
  if (sudoRs && PAM_PASSWORD_WORD.test(sudoRs[1])) return { prompt, user: null }

  if (knownUser && LOCALIZED_SUDO_PROMPT.test(prompt)) {
    const userToken = new RegExp(`(^|[\\s(])${escapeRegExp(knownUser)}($|[\\s):：'’])`)
    if (userToken.test(prompt.slice("[sudo] ".length))) return { prompt, user: knownUser }
  }

  for (const pattern of customPatterns) {
    const match = prompt.match(pattern)
    if (match) return { prompt, user: match.groups?.user ?? null }
  }

  return null
}

/**
 * Reads the text before the cursor on the cursor's line, including earlier
 * rows it wrapped from. This is what the user sees, so colors, cursor moves,
 * and tmux redraws in the raw stream do not matter.
 */
export function readCursorLine(buffer: IBuffer): string {
  let row = buffer.baseY + buffer.cursorY
  let text = buffer.getLine(row)?.translateToString(true, 0, buffer.cursorX) ?? ""
  while (row > 0 && text.length <= MAX_PROMPT_CHARS && buffer.getLine(row)?.isWrapped) {
    row -= 1
    text = (buffer.getLine(row)?.translateToString(false) ?? "") + text
  }
  return text
}
