import type { TFunction } from "i18next"
import { BookMarked, Clock, FileCog, Plus, Server, Terminal } from "lucide-react"

import {
  CONFIG_MATCH_LIMIT,
  PROFILE_MATCH_LIMIT,
  describeConnection,
  type LauncherItem,
} from "@/components/NewTabLauncher/launcherItems"
import type { AppConfig } from "@/lib/appConfig"
import { profileSearchFields } from "@/lib/profileSearch"
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
  shells: readonly TerminalShellProfile[]
  terminalShell: AppConfig["terminal_shell"]
  terminalShellCustomPath: string
  t: TFunction
}

/**
 * The launcher's list for what was typed: with nothing typed, the local
 * terminals, a new connection and the recents; otherwise everything that
 * matches, the typed connection first.
 */
export function buildLauncherItems({
  input,
  configHosts,
  profiles,
  recents,
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

  const recentItems = recents
    .filter((recent) => matchesQuery(query, [recent.title, recent.username, recent.host]))
    .map<LauncherItem>((recent) => ({
      key: `recent:${recent.username}@${recent.host}:${recent.port}:${recent.lastUsedAt}`,
      icon: Clock,
      label: recent.title,
      detail: describeConnection(recent),
      section: "recent",
      action: { kind: "connect", connection: recent },
      recent,
    }))

  const profileItems = query
    ? profiles
        .filter((profile) => matchesQuery(query, profileSearchFields(profile)))
        .slice(0, PROFILE_MATCH_LIMIT)
        .map<LauncherItem>((profile) => ({
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
        }))
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
    return [...terminalItems, newConnectionItem, ...recentItems]
  }
  return [
    ...(connectItem ? [connectItem] : []),
    ...recentItems,
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
