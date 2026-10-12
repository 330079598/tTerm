import type { LucideIcon } from "lucide-react"

import {
  formatEndpoint,
  type QuickConnection,
  type RecentQuickConnection,
} from "@/lib/quickConnect"
import type { SavedProfile, TerminalShellType } from "@/types/tab"

export const PROFILE_MATCH_LIMIT = 8
export const CONFIG_MATCH_LIMIT = 6

type LauncherAction =
  | { kind: "connect"; connection: QuickConnection }
  /** A host was typed without a user name: put the cursor where it goes. */
  | { kind: "add-user"; input: string; caret: number }
  | { kind: "profile"; profile: SavedProfile }
  /** A local terminal; without a shell, the default one from the settings. */
  | { kind: "terminal"; shell?: LocalShellChoice }
  | { kind: "new-connection" }

/** A local shell other than the default one. */
export interface LocalShellChoice {
  shell: TerminalShellType
  /** The executable of a detected `custom` shell. */
  customPath?: string
}

export type LauncherSection = "localTerminal" | "recent" | "profiles" | "sshConfig"

export interface LauncherItem {
  key: string
  icon: LucideIcon
  label: string
  detail?: string
  section?: LauncherSection
  action: LauncherAction
  /** Recents can be forgotten from the list. */
  recent?: RecentQuickConnection
}

export function describeConnection(connection: QuickConnection): string {
  const endpoint = formatEndpoint(connection)
  const jumps = connection.jumpHosts?.map((jump) => formatEndpoint(jump)) ?? []
  return [...jumps, endpoint].join(" → ")
}
