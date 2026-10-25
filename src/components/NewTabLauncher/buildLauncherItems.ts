import type { TFunction } from "i18next"
import { BookMarked, Clock, FileCog, Plus, Server, Terminal } from "lucide-react"

import {
  CONFIG_MATCH_LIMIT,
  PROFILE_MATCH_LIMIT,
  RECENT_ITEMS_LIMIT,
  describeConnection,
  type LauncherItem,
} from "@/components/NewTabLauncher/launcherItems"
import type { AppConfig } from "@/lib/appConfig"
import { profileLastUsed, type RecentProfile } from "@/lib/profileRecents"
import { profileSearchFields, rankProfiles } from "@/lib/profileSearch"
import {
  formatEndpoint,
  matchesQuery,
  parseQuickConnectInput,
  quickConnectionFromConfig,
  resolveQuickConnectTarget,
  type RecentQuickConnection,
  type SshConfigHost,
} from "@/lib/quickConnect"
import {
  isDetectedCustomShell,
  terminalShellLabel,
  type TerminalShellProfile,
} from "@/lib/terminalShells"
import type { SavedProfile } from "@/types/tab"

/** What the launcher's list is built from. */
export interface LauncherItemsInput {
  /** What was typed into the launcher. */
  input: string
  configHosts: readonly SshConfigHost[]
  profiles: readonly SavedProfile[]
  recents: readonly RecentQuickConnection[]
  /** The saved connections used lately, newest first. */
  profileRecents: readonly RecentProfile[]
  shells: readonly TerminalShellProfile[]
  terminalShell: AppConfig["terminal_shell"]
  terminalShellCustomPath: string
  t: TFunction
}

/**
 * The launcher's list for what was typed: with nothing typed, the local
 * terminals, a new connection and the recents, quick and saved connections
 * together; otherwise everything that matches, the typed connection first
 * and the saved connections the search fits best, then those used lately,
 * before the others.
 */
export function buildLauncherItems({
  input,
  configHosts,
  profiles,
  recents,
  profileRecents,
  shells,
  terminalShell,
  terminalShellCustomPath,
  t,
}: LauncherItemsInput): LauncherItem[] {
  const query = input.trim()
  const target = parseQuickConnectInput(query)
  const resolved = target ? resolveQuickConnectTarget(target, configHosts) : null

  const connectItem: LauncherItem | null = resolved
    ? {
        key: "typed",
        icon: Server,
        label: t("quickConnect.connectTo", { target: describeConnection(resolved) }),
        action: { kind: "connect", connection: resolved },
      }
    : null
  // `ssh -p 2222 host` gets `-l` appended; a bare host gets `@` in front.
  const addUserItem: LauncherItem | null =
    target && !resolved
      ? {
          key: "typed-add-user",
          icon: Server,
          label: t("quickConnect.connectTo", { target: formatEndpoint(target) }),
          detail: t("quickConnect.addUser"),
          action: /^ssh\s|^-/.test(query)
            ? { kind: "add-user", input: `${query} -l `, caret: query.length + 4 }
            : { kind: "add-user", input: `@${query}`, caret: 0 },
        }
      : null

  // The default shell first, then every other shell found on this machine.
  const defaultShell = terminalShell
  const defaultCustomPath = terminalShellCustomPath.trim()
  const defaultShellProfile = shells.find((profile) => profile.shell === defaultShell)
  const defaultShellLabel =
    defaultShell === "custom"
      ? defaultCustomPath || t("connection.terminalShellOptions.custom")
      : defaultShellProfile
        ? terminalShellLabel(defaultShellProfile, t)
        : defaultShell
  const terminalItems: LauncherItem[] = [
    {
      key: "terminal",
      icon: Terminal,
      label: defaultShellLabel,
      detail: t("quickConnect.defaultShell"),
      section: "localTerminal",
      action: { kind: "terminal" },
    },
    ...shells
      .filter((profile) =>
        profile.shell === "custom"
          ? isDetectedCustomShell(profile) &&
            !(defaultShell === "custom" && profile.source === defaultCustomPath)
          : profile.shell !== "auto" && profile.shell !== defaultShell
      )
      .map<LauncherItem>((profile) => ({
        key: `terminal:${profile.shell}:${profile.source}`,
        icon: Terminal,
        label: terminalShellLabel(profile, t),
        detail: profile.source,
        section: "localTerminal",
        action: {
          kind: "terminal",
          shell: {
            shell: profile.shell,
            customPath: profile.shell === "custom" ? profile.source : undefined,
          },
        },
      })),
  ]
  const newConnectionItem: LauncherItem = {
    key: "new-connection",
    icon: Plus,
    label: t("quickConnect.newConnection"),
    detail: query
      ? t("quickConnect.newConnectionPrefilled")
      : t("quickConnect.newConnectionDetail"),
    action: { kind: "new-connection" },
  }

  const quickRecents = recents
    .filter((recent) => matchesQuery(query, [recent.title, recent.username, recent.host]))
    .map((recent) => ({
      usedAt: recent.lastUsedAt,
      item: {
        key: `recent:${recent.username}@${recent.host}:${recent.port}:${recent.lastUsedAt}`,
        icon: Clock,
        label: recent.title,
        detail: describeConnection(recent),
        section: "recent",
        action: { kind: "connect", connection: recent },
        recent: { kind: "quick", connection: recent },
      } satisfies LauncherItem,
    }))

  const profileItem = (profile: SavedProfile): LauncherItem => ({
    key: `profile:${profile.id}`,
    icon: BookMarked,
    label: profile.name,
    detail: [
      profile.group,
      profile.connection_type === "serial"
        ? profile.serial?.port
        : formatEndpoint({
            username: profile.username,
            host: profile.host ?? "",
            port: profile.port,
          }),
    ]
      .filter(Boolean)
      .join(" · "),
    section: "profiles",
    action: { kind: "profile", profile },
  })

  const lastUsed = profileLastUsed(profileRecents)
  // Ranked before the cut, so the best fit is never the one left out.
  const profileItems = query
    ? rankProfiles(
        profiles.filter((profile) => matchesQuery(query, profileSearchFields(profile))),
        query,
        lastUsed
      )
        .slice(0, PROFILE_MATCH_LIMIT)
        .map(profileItem)
    : []

  const configItems = query
    ? configHosts
        .filter((host) => matchesQuery(query, [host.hostPattern, host.host, host.username]))
        // Exactly the typed alias is the typed item already.
        .filter((host) => !(resolved && target?.host === host.hostPattern))
        .slice(0, CONFIG_MATCH_LIMIT)
        .map<LauncherItem>((host) => {
          const connection = quickConnectionFromConfig(host)
          return {
            key: `config:${host.hostPattern}`,
            icon: FileCog,
            label: host.hostPattern,
            detail: connection
              ? describeConnection(connection)
              : `${formatEndpoint({ host: host.host ?? host.hostPattern, port: host.port })} · ${t("quickConnect.addUser")}`,
            section: "sshConfig",
            action: connection
              ? { kind: "connect", connection }
              : { kind: "add-user", input: `@${host.hostPattern}`, caret: 0 },
          }
        })
    : []

  if (!query) {
    const profilesById = new Map(profiles.map((profile) => [profile.id, profile]))
    // A recent whose profile was deleted since is left out.
    const savedRecents = profileRecents.flatMap((recent) => {
      const profile = profilesById.get(recent.profileId)
      if (!profile) return []
      const item: LauncherItem = {
        ...profileItem(profile),
        key: `recent-profile:${profile.id}`,
        section: "recent",
        recent: { kind: "profile", profileId: profile.id },
      }
      return [{ usedAt: recent.lastUsedAt, item }]
    })
    const recentItems = [...quickRecents, ...savedRecents]
      .sort((left, right) => right.usedAt - left.usedAt)
      .slice(0, RECENT_ITEMS_LIMIT)
      .map((recent) => recent.item)
    return [...terminalItems, newConnectionItem, ...recentItems]
  }
  return [
    ...(connectItem ? [connectItem] : []),
    ...quickRecents.map((recent) => recent.item),
    ...profileItems,
    ...configItems,
    ...(addUserItem ? [addUserItem] : []),
    ...terminalItems.filter((item) =>
      matchesQuery(query, [
        item.label,
        item.detail,
        t("quickConnect.localTerminal"),
        "terminal local shell",
      ])
    ),
    newConnectionItem,
  ]
}
