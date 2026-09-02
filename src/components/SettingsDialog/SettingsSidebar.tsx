import React from "react"
import {
  FileClock,
  FolderTree,
  HardDriveDownload,
  Info,
  Palette,
  Plug,
  Shield,
  SquareTerminal,
  UploadCloud,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { cn } from "@/lib/utils"

interface SettingsSidebarProps {
  activeTab: string
  onTabChange: (value: string) => void
}

export const SettingsSidebar: React.FC<SettingsSidebarProps> = ({ activeTab, onTabChange }) => {
  const { t } = useTranslation()
  const itemRefs = React.useRef<Array<HTMLButtonElement | null>>([])

  const groups = [
    {
      label: t("settings.categories.workspace", { defaultValue: "Workspace" }),
      items: [
        {
          value: "profile-groups",
          label: t("settings.profileGroups", { defaultValue: "Groups" }),
          icon: FolderTree,
        },
        {
          value: "connection",
          label: t("settings.connection", { defaultValue: "Connection" }),
          icon: Plug,
        },
      ],
    },
    {
      label: t("settings.categories.experience", { defaultValue: "Experience" }),
      items: [
        { value: "appearance", label: t("settings.appearance"), icon: Palette },
        {
          value: "terminal",
          label: t("settings.terminal", { defaultValue: "Terminal" }),
          icon: SquareTerminal,
        },
      ],
    },
    {
      label: t("settings.categories.data", { defaultValue: "Data & security" }),
      items: [
        { value: "logging", label: t("terminalLogging.title"), icon: FileClock },
        { value: "data-migration", label: t("dataMigration.sidebar"), icon: HardDriveDownload },
        { value: "security", label: t("settings.security"), icon: Shield },
      ],
    },
    {
      label: t("settings.categories.application", { defaultValue: "Application" }),
      items: [
        { value: "general", label: t("settings.general"), icon: Info },
        {
          value: "updates",
          label: t("settings.updates", { defaultValue: "Updates" }),
          icon: UploadCloud,
        },
      ],
    },
  ]
  const items = groups.flatMap((group) => group.items)

  const handleKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    let nextIndex = index
    if (event.key === "ArrowDown") nextIndex = (index + 1) % items.length
    else if (event.key === "ArrowUp") nextIndex = (index - 1 + items.length) % items.length
    else if (event.key === "Home") nextIndex = 0
    else if (event.key === "End") nextIndex = items.length - 1
    else return

    event.preventDefault()
    onTabChange(items[nextIndex].value)
    itemRefs.current[nextIndex]?.focus()
  }

  let itemIndex = 0

  return (
    <aside className="border-border bg-muted/30 w-14 shrink-0 border-r px-2 py-3 sm:w-48 sm:px-3">
      <nav aria-label={t("settings.title")}>
        <div role="tablist" aria-orientation="vertical" className="space-y-4">
          {groups.map((group) => (
            <div key={group.label} role="presentation">
              <div
                aria-hidden="true"
                className="text-muted-foreground mb-1 hidden px-2 text-[10px] font-semibold uppercase sm:block"
              >
                {group.label}
              </div>
              <div role="presentation" className="space-y-0.5">
                {group.items.map((item) => {
                  const index = itemIndex++
                  const isActive = item.value === activeTab
                  const Icon = item.icon

                  return (
                    <button
                      key={item.value}
                      ref={(element) => {
                        itemRefs.current[index] = element
                      }}
                      id={`settings-tab-${item.value}`}
                      type="button"
                      role="tab"
                      aria-controls={`settings-panel-${item.value}`}
                      aria-selected={isActive}
                      tabIndex={isActive ? 0 : -1}
                      title={item.label}
                      onClick={() => onTabChange(item.value)}
                      onKeyDown={(event) => handleKeyDown(event, index)}
                      className={cn(
                        "focus-visible:ring-ring flex h-8 w-full cursor-pointer items-center justify-center gap-2 rounded-md px-2 text-[13px] font-medium transition-colors duration-200 outline-none focus-visible:ring-2 sm:justify-start",
                        isActive
                          ? "bg-background text-foreground shadow-sm"
                          : "text-muted-foreground hover:bg-background/60 hover:text-foreground"
                      )}
                    >
                      <Icon size={16} className="shrink-0" aria-hidden="true" />
                      <span className="hidden truncate sm:block">{item.label}</span>
                    </button>
                  )
                })}
              </div>
            </div>
          ))}
        </div>
      </nav>
    </aside>
  )
}
