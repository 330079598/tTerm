// Notifications programs in the terminal ask for, the end of long commands
// and what AI agents report, which tTerm announces when the user is not
// looking at the tab.

import type { AgentAnnouncement } from "@/lib/agentStatus"

/** OSC codes that carry a notification: iTerm2's 9, urxvt's 777, kitty's 99. */
export const NOTIFICATION_OSC_CODES = [9, 777, 99] as const

export type NotificationOscCode = (typeof NOTIFICATION_OSC_CODES)[number]

export interface TerminalNotificationRequest {
  title?: string
  body: string
}

/** Something in a terminal that may deserve the user's attention. */
export type TerminalAttentionEvent =
  | { kind: "command"; command?: string; exitCode?: number; durationMs: number }
  | ({ kind: "request" } & TerminalNotificationRequest)
  | ({ kind: "agent" } & AgentAnnouncement)
  | { kind: "bell" }

const MAX_TITLE_LENGTH = 256
const MAX_BODY_LENGTH = 1024

function clean(text: string, maxLength: number) {
  // eslint-disable-next-line no-control-regex
  const visible = text.replace(/[\u0000-\u001f\u007f-\u009f]+/g, " ").trim()
  return visible.length > maxLength ? `${visible.slice(0, maxLength - 1)}…` : visible
}

function request(title: string | undefined, body: string): TerminalNotificationRequest | null {
  const cleanTitle = title === undefined ? "" : clean(title, MAX_TITLE_LENGTH)
  const cleanBody = clean(body, MAX_BODY_LENGTH)
  if (!cleanTitle && !cleanBody) return null
  // A notification with only a title shows it as the message.
  if (!cleanBody) return { body: cleanTitle }
  return cleanTitle ? { title: cleanTitle, body: cleanBody } : { body: cleanBody }
}

/**
 * OSC 9 `message`. ConEmu numbers its own OSC 9 commands (`9;4;…` progress,
 * `9;9;…` working directory), so a payload that starts with a number and a
 * semicolon is one of those, not a message.
 */
export function parseOsc9Notification(data: string): TerminalNotificationRequest | null {
  if (/^\d+(;|$)/.test(data)) return null
  return request(undefined, data)
}

/** OSC 777 `notify;title;body`; the body may itself contain semicolons. */
export function parseOsc777Notification(data: string): TerminalNotificationRequest | null {
  const [command, title = "", ...body] = data.split(";")
  if (command !== "notify") return null
  return request(title, body.join(";"))
}

function decodeBase64Utf8(text: string): string | null {
  try {
    const bytes = Uint8Array.from(atob(text), (char) => char.charCodeAt(0))
    return new TextDecoder().decode(bytes)
  } catch {
    return null
  }
}

type KittyPartial = { title: string; body: string }

/**
 * kitty's OSC 99 `metadata;payload`. A notification may arrive in chunks that
 * share an `i` id, all but the last with `d=0`; `p` says whether a chunk is
 * the title (the default) or the body, `e=1` that it is base64. Queries,
 * icons, buttons and close requests are ignored.
 */
export class KittyNotificationAssembler {
  private readonly partials = new Map<string, KittyPartial>()

  handle(data: string): TerminalNotificationRequest | null {
    const separator = data.indexOf(";")
    if (separator < 0) return null
    const metadata = new Map<string, string>()
    for (const pair of data.slice(0, separator).split(":")) {
      const equals = pair.indexOf("=")
      if (equals > 0) metadata.set(pair.slice(0, equals), pair.slice(equals + 1))
    }
    const id = metadata.get("i") ?? ""
    const part = metadata.get("p") ?? "title"
    let payload = data.slice(separator + 1)
    if (metadata.get("e") === "1") {
      const decoded = decodeBase64Utf8(payload)
      if (decoded === null) return null
      payload = decoded
    }

    const partial = this.partials.get(id) ?? { title: "", body: "" }
    if (part === "title") partial.title += payload
    else if (part === "body") partial.body += payload
    else return null

    if (metadata.get("d") === "0") {
      // Bounded: a program that never finishes its chunks cannot grow this.
      if (!this.partials.has(id) && this.partials.size >= 16) {
        const oldest = this.partials.keys().next().value
        if (oldest !== undefined) this.partials.delete(oldest)
      }
      this.partials.set(id, partial)
      return null
    }
    this.partials.delete(id)
    return partial.body ? request(partial.title, partial.body) : request(undefined, partial.title)
  }
}

export interface FinishedCommand {
  command?: string
  exitCode?: number
  durationMs: number
}

/**
 * Times commands from shell integration marks: a command starts at Enter on
 * its prompt or at its output mark (C), whichever comes first, and finishes
 * at its done mark (D). A new prompt (A) without a done mark forgets it.
 */
export class CommandTimer {
  private startedAt: number | null = null
  private command: string | undefined

  prompt() {
    this.startedAt = null
    this.command = undefined
  }

  start(now: number) {
    if (this.startedAt === null) this.startedAt = now
  }

  setCommand(command: string) {
    this.command = command
  }

  finish(exitCode: number | undefined, now: number): FinishedCommand | null {
    const startedAt = this.startedAt
    const command = this.command
    this.prompt()
    if (startedAt === null) return null
    return { command, exitCode, durationMs: Math.max(0, now - startedAt) }
  }
}

/** How much a tab wants to be looked at, shown on its tab. */
export type TabAttentionLevel = "activity" | "bell" | "notification"

const ATTENTION_RANK: Record<TabAttentionLevel, number> = {
  activity: 1,
  bell: 2,
  notification: 3,
}

export function strongerAttention(
  current: TabAttentionLevel | undefined,
  next: TabAttentionLevel
): TabAttentionLevel {
  return current && ATTENTION_RANK[current] >= ATTENTION_RANK[next] ? current : next
}

export interface NotificationSettings {
  notifications_enabled: boolean
  notify_command_finished: boolean
  notify_command_min_secs: number
  notify_terminal_requests: boolean
  notify_agent_waiting: boolean
  notify_agent_done: boolean
  bell_notify: boolean
}

export interface AttentionDecision {
  /** Mark on the tab, when it is out of sight. */
  mark: TabAttentionLevel | null
  /** A system notification: the window is not in front. */
  system: boolean
  /** An in-app toast: the window is in front but the tab is out of sight. */
  toast: boolean
}

const NOTHING: AttentionDecision = { mark: null, system: false, toast: false }

/**
 * What an event leads to. Nothing interrupts a user who is looking at the
 * tab; otherwise the tab is marked, and the event is announced in the app or
 * by the system depending on whether the window is in front.
 */
export function decideAttention(
  event: TerminalAttentionEvent,
  settings: NotificationSettings,
  { windowFocused, tabVisible }: { windowFocused: boolean; tabVisible: boolean }
): AttentionDecision {
  if (windowFocused && tabVisible) return NOTHING
  const mark = tabVisible ? null : event.kind === "bell" ? "bell" : "notification"

  let announce: boolean
  if (event.kind === "command") {
    if (
      !settings.notify_command_finished ||
      event.durationMs < settings.notify_command_min_secs * 1000
    ) {
      return NOTHING
    }
    announce = settings.notifications_enabled
  } else if (event.kind === "request") {
    if (!settings.notify_terminal_requests) return NOTHING
    announce = settings.notifications_enabled
  } else if (event.kind === "agent") {
    const wanted =
      event.state === "waiting" ? settings.notify_agent_waiting : settings.notify_agent_done
    if (!wanted) return NOTHING
    announce = settings.notifications_enabled
  } else {
    // A bell marks its tab even when it announces nothing.
    announce = settings.notifications_enabled && settings.bell_notify
  }

  return {
    mark,
    system: announce && !windowFocused,
    toast: announce && windowFocused && event.kind !== "bell",
  }
}

/** `1h 2m`, `3m 5s` or `42s`. */
export function formatCommandDuration(durationMs: number): string {
  const totalSeconds = Math.round(durationMs / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) return `${hours}h ${minutes}m`
  if (minutes > 0) return `${minutes}m ${seconds}s`
  return `${seconds}s`
}
