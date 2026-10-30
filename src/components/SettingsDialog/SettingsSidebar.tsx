import React from "react"
import { useTranslation } from "react-i18next"

import { SETTINGS_TAB_GROUPS } from "@/components/SettingsDialog/settingsTabs"
import { cn } from "@/lib/utils"

interface SettingsSidebarProps {
  activeTab: string
  onTabChange: (value: string) => void
}

export const SettingsSidebar: React.FC<SettingsSidebarProps> = ({ activeTab, onTabChange }) => {
  const { t } = useTranslation()
  const itemRefs = React.useRef<Array<HTMLButtonElement | null>>([])

  const groups = SETTINGS_TAB_GROUPS.map((group) => ({
    label: t(group.labelKey, { defaultValue: group.defaultLabel }),
    items: group.items.map((item) => ({
      value: item.value,
      label: t(item.labelKey, { defaultValue: item.defaultLabel }),
      icon: item.icon,
    })),
  }))
  const items = groups.flatMap((group) => group.items)

  const handleKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    let nextIndex: number
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
    <aside className="border-border bg-muted/30 min-h-0 w-14 shrink-0 overflow-y-auto border-r px-2 py-3 sm:w-48 sm:px-3">
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
