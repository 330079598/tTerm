import React from "react"
import { Shapes } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { TabIconGlyph } from "@/components/TabIcon"
import { TabIconPicker } from "@/components/TabIconPicker"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { getDetectedPlatform } from "@/lib/uiPlatform"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { useSyncedState } from "@/hooks/useSyncedState"
import { programName } from "@/lib/localShells"
import { detectLocalShellName } from "@/lib/localTabTitle"
import {
  applyTabIconStyle,
  builtInShellIcon,
  shellDisplayName,
  shellsForSettings,
  withShellIconStyle,
  type TabIconConfig,
} from "@/lib/tabIcons"

// A custom color reports every step while its picker is dragged; saving waits
// for it to settle.
const SAVE_DELAY_MS = 400

/** Settings for the icons on terminal tabs: whether they show, and each shell's. */
export function TabIconSettings() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [icons, setIcons] = useSyncedState(config.tab_icons)
  const [editing, setEditing] = React.useState<string | null>(null)
  const [defaultShell, setDefaultShell] = React.useState<string | null>(null)
  const { terminal_shell: terminalShell, terminal_shell_custom_path: customPath } = config
  const pendingRef = React.useRef<{ timer: number; next: TabIconConfig } | null>(null)

  const flush = React.useCallback(() => {
    const pending = pendingRef.current
    if (!pending) return
    window.clearTimeout(pending.timer)
    pendingRef.current = null
    void saveSettings({ tab_icons: pending.next })
  }, [saveSettings])

  React.useEffect(() => flush, [flush])

  // The default shell may be one the built-in list leaves out, such as nu or xonsh.
  React.useEffect(() => {
    let current = true
    void detectLocalShellName({
      terminalShell,
      terminalShellCustomPath: customPath,
    }).then((name) => {
      if (current) setDefaultShell(name)
    })
    return () => {
      current = false
    }
  }, [customPath, terminalShell])

  const save = (next: TabIconConfig) => {
    setIcons(next)
    if (pendingRef.current) window.clearTimeout(pendingRef.current.timer)
    pendingRef.current = { timer: window.setTimeout(flush, SAVE_DELAY_MS), next }
  }

  return (
    <SettingsSection
      icon={<Shapes size={16} />}
      title={t("tabIcons.title")}
      description={t("tabIcons.description")}
    >
      <SettingsRow
        title={t("tabIcons.show")}
        description={t("tabIcons.showDescription")}
        action={
          <Switch
            checked={icons.enabled}
            aria-label={t("tabIcons.show")}
            onCheckedChange={(enabled) => save({ ...icons, enabled })}
          />
        }
      />
      {icons.enabled && (
        <SettingsRow title={t("tabIcons.byShell")} description={t("tabIcons.byShellDescription")}>
          <div className="divide-border divide-y">
            {shellsForSettings(getDetectedPlatform(), icons, [
              defaultShell,
              ...config.local_shells.map((shell) => programName(shell.path)),
            ]).map((key) => {
              const builtIn = builtInShellIcon(key)
              const style = icons.shells[key]
              const open = editing === key
              return (
                <div key={key} className="py-2 first:pt-0 last:pb-0">
                  <div className="flex items-center gap-3">
                    <TabIconGlyph icon={applyTabIconStyle(builtIn, style)} />
                    <span className="min-w-0 flex-1 truncate text-sm">{shellDisplayName(key)}</span>
                    <Button
                      type="button"
                      variant="outline"
                      size="xs"
                      aria-expanded={open}
                      onClick={() => setEditing(open ? null : key)}
                    >
                      {open ? t("tabIcons.done") : t("tabIcons.change")}
                    </Button>
                  </div>
                  {open && (
                    <div className="mt-3 pl-6">
                      <TabIconPicker
                        value={style}
                        inherited={builtIn}
                        resetLabel={t("tabIcons.resetShell")}
                        onChange={(next) => save(withShellIconStyle(icons, key, next))}
                      />
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        </SettingsRow>
      )}
    </SettingsSection>
  )
}
