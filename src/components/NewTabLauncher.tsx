import React, { useEffect, useMemo, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import {
  BookMarked,
  Clock,
  FileCog,
  type LucideIcon,
  Plus,
  Server,
  Terminal,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { useConfig } from "@/contexts/ConfigContext"
import { isImeKeyEvent } from "@/lib/ime"
import { buildConnectionFromProfile } from "@/lib/profileConnections"
import {
  buildQuickConnectTab,
  formatEndpoint,
  matchesQuery,
  parseQuickConnectInput,
  quickConnectionFromConfig,
  resolveQuickConnectTarget,
  type QuickConnection,
  type RecentQuickConnection,
  type SshConfigHost,
} from "@/lib/quickConnect"
import {
  isDetectedCustomShell,
  terminalShellLabel,
  type TerminalShellProfile,
} from "@/lib/terminalShells"
import { cn } from "@/lib/utils"
import type { SavedProfile, Tab, TerminalShellType } from "@/types/tab"

const PROFILE_MATCH_LIMIT = 8
const CONFIG_MATCH_LIMIT = 6

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

type LauncherSection = "localTerminal" | "recent" | "profiles" | "sshConfig"

interface LauncherItem {
  key: string
  icon: LucideIcon
  label: string
  detail?: string
  section?: LauncherSection
  action: LauncherAction
  /** Recents can be forgotten from the list. */
  recent?: RecentQuickConnection
}

interface NewTabLauncherProps {
  open: boolean
  onClose: () => void
  /** Opens a tab: a quick connection, a saved profile. */
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  onOpenLocalTerminal: (shell?: LocalShellChoice) => void
  /** Opens the full connection dialog, prefilled from what was typed. */
  onOpenConnectionDialog: (input: string, configHosts: readonly SshConfigHost[]) => void
  recents: readonly RecentQuickConnection[]
  onForgetRecent: (recent: RecentQuickConnection) => void
  onClearRecents: () => void
}

function describeConnection(connection: QuickConnection): string {
  const endpoint = formatEndpoint(connection)
  const jumps = connection.jumpHosts?.map((jump) => formatEndpoint(jump)) ?? []
  return [...jumps, endpoint].join(" → ")
}

/**
 * What the `+` button opens: a box to type `user@host` into, with recent
 * quick connections, matching saved profiles and `~/.ssh/config` hosts, a
 * local terminal and the full connection dialog.
 */
export const NewTabLauncher: React.FC<NewTabLauncherProps> = ({
  open,
  onClose,
  onConnect,
  onOpenLocalTerminal,
  onOpenConnectionDialog,
  recents,
  onForgetRecent,
  onClearRecents,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [input, setInput] = useState("")
  const [selected, setSelected] = useState(0)
  const [profiles, setProfiles] = useState<SavedProfile[]>([])
  const [configHosts, setConfigHosts] = useState<SshConfigHost[]>([])
  const [shells, setShells] = useState<TerminalShellProfile[]>([])
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const pendingCaretRef = useRef<number | null>(null)

  useEffect(() => {
    if (!open) return
    let cancelled = false
    invoke<SavedProfile[]>("list_profiles")
      .then((loaded) => {
        if (!cancelled) setProfiles(loaded.filter((profile) => profile.connection_type === "ssh"))
      })
      .catch(() => {})
    // No ~/.ssh/config is the common case, not an error.
    invoke<{ hosts: SshConfigHost[] }>("preview_ssh_config_import", { sourcePath: null })
      .then((preview) => {
        if (!cancelled) setConfigHosts(preview.hosts.filter((host) => !host.skipped))
      })
      .catch(() => {})
    invoke<TerminalShellProfile[]>("list_available_terminal_shells")
      .then((detected) => {
        if (!cancelled) setShells(detected)
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [open])

  const items = useMemo<LauncherItem[]>(() => {
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
    const defaultShell = config.terminal_shell
    const defaultCustomPath = config.terminal_shell_custom_path.trim()
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
          .filter((profile) =>
            matchesQuery(query, [profile.name, profile.host, profile.username, profile.group])
          )
          .slice(0, PROFILE_MATCH_LIMIT)
          .map<LauncherItem>((profile) => ({
            key: `profile:${profile.id}`,
            icon: BookMarked,
            label: profile.name,
            detail: [
              profile.group,
              formatEndpoint({
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
  }, [
    config.terminal_shell,
    config.terminal_shell_custom_path,
    configHosts,
    input,
    profiles,
    recents,
    shells,
    t,
  ])

  const selectedIndex = Math.min(selected, items.length - 1)

  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(`[data-index="${selectedIndex}"]`)
      ?.scrollIntoView({ block: "nearest" })
  }, [selectedIndex])

  useEffect(() => {
    const caret = pendingCaretRef.current
    if (caret === null) return
    pendingCaretRef.current = null
    inputRef.current?.setSelectionRange(caret, caret)
  }, [input])

  const close = () => {
    setInput("")
    setSelected(0)
    onClose()
  }

  const run = (item: LauncherItem) => {
    const action = item.action
    switch (action.kind) {
      case "connect":
        onConnect(buildQuickConnectTab(action.connection))
        break
      case "profile":
        onConnect(buildConnectionFromProfile(action.profile))
        break
      case "terminal":
        onOpenLocalTerminal(action.shell)
        break
      case "new-connection":
        onOpenConnectionDialog(input.trim(), configHosts)
        break
      case "add-user":
        pendingCaretRef.current = action.caret
        setInput(action.input)
        setSelected(0)
        inputRef.current?.focus()
        return
    }
    close()
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (isImeKeyEvent(event.nativeEvent)) return
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault()
      const step = event.key === "ArrowDown" ? 1 : -1
      setSelected((items.length + selectedIndex + step) % items.length)
    } else if (event.key === "Enter") {
      event.preventDefault()
      const item = items[selectedIndex]
      if (item) run(item)
    } else if (event.key === "Delete" && event.shiftKey) {
      // Like a browser's address bar: Shift+Delete forgets the chosen recent.
      const recent = items[selectedIndex]?.recent
      if (!recent) return
      event.preventDefault()
      onForgetRecent(recent)
    }
  }

  const clearRecents = async () => {
    const confirmed = await confirm({
      title: t("quickConnect.clearRecentsTitle"),
      description: t("quickConnect.clearRecentsDescription", { count: recents.length }),
      confirmText: t("quickConnect.clearRecents"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (confirmed) onClearRecents()
    inputRef.current?.focus()
  }

  const sectionLabels: Record<LauncherSection, string> = {
    localTerminal: t("quickConnect.sections.localTerminal"),
    recent: t("quickConnect.sections.recent"),
    profiles: t("quickConnect.sections.profiles"),
    sshConfig: t("quickConnect.sections.sshConfig"),
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) close()
      }}
    >
      <DialogContent
        className="top-[15%] translate-y-0 gap-0 overflow-hidden p-0 sm:max-w-[560px]"
        showCloseButton={false}
      >
        <DialogTitle className="sr-only">{t("tabs.newTab")}</DialogTitle>
        <div className="border-b px-3 py-2.5">
          <input
            ref={inputRef}
            data-dialog-initial-focus
            value={input}
            onChange={(event) => {
              setInput(event.target.value)
              setSelected(0)
            }}
            onKeyDown={handleKeyDown}
            placeholder={t("quickConnect.placeholder")}
            aria-label={t("quickConnect.placeholder")}
            role="combobox"
            aria-expanded="true"
            aria-controls="new-tab-launcher-list"
            aria-activedescendant={
              items[selectedIndex] ? `new-tab-launcher-${selectedIndex}` : undefined
            }
            autoComplete="off"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            className="placeholder:text-muted-foreground w-full bg-transparent font-mono text-sm outline-none"
          />
        </div>
        <div
          ref={listRef}
          id="new-tab-launcher-list"
          role="listbox"
          className="max-h-[min(420px,60vh)] overflow-y-auto p-1.5"
        >
          {items.map((item, index) => {
            const Icon = item.icon
            const showSection = item.section && items[index - 1]?.section !== item.section
            return (
              <React.Fragment key={item.key}>
                {showSection && item.section && (
                  <div className="text-muted-foreground flex items-center px-2 pt-2 pb-1 text-[11px] font-medium tracking-wide uppercase">
                    <span className="flex-1">{sectionLabels[item.section]}</span>
                    {/* Filtered, the list may not show every recent the button clears. */}
                    {item.section === "recent" && !input.trim() && (
                      <button
                        type="button"
                        tabIndex={-1}
                        onMouseDown={(event) => event.preventDefault()}
                        onClick={() => void clearRecents()}
                        className="hover:text-foreground rounded px-1 normal-case"
                      >
                        {t("quickConnect.clearRecents")}
                      </button>
                    )}
                  </div>
                )}
                <div
                  id={`new-tab-launcher-${index}`}
                  data-index={index}
                  role="option"
                  aria-selected={index === selectedIndex}
                  onMouseMove={() => {
                    if (index !== selectedIndex) setSelected(index)
                  }}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => run(item)}
                  className={cn(
                    "group flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-1.5 text-sm",
                    index === selectedIndex && "bg-accent text-accent-foreground"
                  )}
                >
                  <Icon size={15} className="text-muted-foreground shrink-0" />
                  <span className="min-w-0 shrink-0 truncate">{item.label}</span>
                  {item.detail && (
                    <span className="text-muted-foreground min-w-0 flex-1 truncate font-mono text-xs">
                      {item.detail}
                    </span>
                  )}
                  {!item.detail && <span className="flex-1" />}
                  {item.recent && (
                    <button
                      type="button"
                      tabIndex={-1}
                      aria-label={t("quickConnect.forgetRecent")}
                      title={`${t("quickConnect.forgetRecent")} (Shift+Delete)`}
                      onClick={(event) => {
                        event.stopPropagation()
                        if (item.recent) onForgetRecent(item.recent)
                      }}
                      className={cn(
                        "text-muted-foreground hover:text-foreground rounded p-0.5",
                        index === selectedIndex ? "visible" : "invisible group-hover:visible"
                      )}
                    >
                      <X size={13} />
                    </button>
                  )}
                  {index === selectedIndex && (
                    <kbd className="text-muted-foreground shrink-0 text-[11px]">↵</kbd>
                  )}
                </div>
              </React.Fragment>
            )
          })}
        </div>
        <div className="text-muted-foreground border-t px-3 py-1.5 text-[11px]">
          {items[selectedIndex]?.recent ? t("quickConnect.hintRecent") : t("quickConnect.hint")}
        </div>
        {/* Inside the launcher, so Escape and outside clicks close only the confirmation. */}
        <ConfirmDialog />
      </DialogContent>
    </Dialog>
  )
}
