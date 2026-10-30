import {
  Bell,
  FileClock,
  FolderTree,
  HardDriveDownload,
  Highlighter,
  Info,
  Keyboard,
  type LucideIcon,
  Network,
  Palette,
  Plug,
  Shield,
  SquareTerminal,
  Terminal,
  UploadCloud,
} from "lucide-react"

/** One settings page as the sidebar lists it. */
export interface SettingsTabDef {
  value: string
  labelKey: string
  defaultLabel?: string
  icon: LucideIcon
}

export interface SettingsTabGroup {
  labelKey: string
  defaultLabel: string
  items: SettingsTabDef[]
}

/** The settings pages in sidebar order, which the search shares to name each page. */
export const SETTINGS_TAB_GROUPS: SettingsTabGroup[] = [
  {
    labelKey: "settings.categories.workspace",
    defaultLabel: "Workspace",
    items: [
      {
        value: "profile-groups",
        labelKey: "settings.profileGroups",
        defaultLabel: "Groups",
        icon: FolderTree,
      },
      {
        value: "connection",
        labelKey: "settings.connection",
        defaultLabel: "Connection",
        icon: Plug,
      },
      { value: "proxies", labelKey: "proxies.sidebar", icon: Network },
    ],
  },
  {
    labelKey: "settings.categories.experience",
    defaultLabel: "Experience",
    items: [
      { value: "appearance", labelKey: "settings.appearance", icon: Palette },
      {
        value: "terminal",
        labelKey: "settings.terminal",
        defaultLabel: "Terminal",
        icon: SquareTerminal,
      },
      { value: "shells", labelKey: "localShells.sidebar", icon: Terminal },
      { value: "highlight", labelKey: "keywordHighlight.sidebar", icon: Highlighter },
      { value: "notifications", labelKey: "notifications.sidebar", icon: Bell },
      { value: "keymap", labelKey: "settings.keymap", icon: Keyboard },
    ],
  },
  {
    labelKey: "settings.categories.data",
    defaultLabel: "Data & security",
    items: [
      { value: "logging", labelKey: "terminalLogging.title", icon: FileClock },
      { value: "data-migration", labelKey: "dataMigration.sidebar", icon: HardDriveDownload },
      { value: "security", labelKey: "settings.security", icon: Shield },
    ],
  },
  {
    labelKey: "settings.categories.application",
    defaultLabel: "Application",
    items: [
      { value: "general", labelKey: "settings.general", icon: Info },
      {
        value: "updates",
        labelKey: "settings.updates",
        defaultLabel: "Updates",
        icon: UploadCloud,
      },
    ],
  },
]

export const SETTINGS_TABS: SettingsTabDef[] = SETTINGS_TAB_GROUPS.flatMap((group) => group.items)
