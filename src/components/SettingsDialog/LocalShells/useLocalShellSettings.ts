import { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import type { AppConfig, LocalShell } from "@/lib/appConfig"
import {
  buildShellEntries,
  defaultShellKey,
  defaultShellUpdates,
  movedLocalShell,
  removedLocalShell,
  savedLocalShell,
  toggledHiddenShell,
  type ShellEntry,
} from "@/lib/localShells"
import type { TerminalShellProfile } from "@/lib/terminalShells"

/**
 * The shells for the settings' list, which of them is the default, which of
 * the user's own can't be found here, and the changes the list makes.
 */
export function useLocalShellSettings() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [detected, setDetected] = useState<TerminalShellProfile[]>([])
  // Whether each of the user's shells' paths leads to a program here, by path.
  const [found, setFound] = useState<Record<string, boolean>>({})
  const shells = config.local_shells

  useEffect(() => {
    let current = true
    invoke<TerminalShellProfile[]>("list_available_terminal_shells")
      .then((profiles) => {
        if (current) setDetected(profiles)
      })
      .catch((error) => console.warn("Failed to list the local shells", error))
    return () => {
      current = false
    }
  }, [])

  useEffect(() => {
    let current = true
    for (const path of new Set(shells.map((shell) => shell.path.trim()))) {
      invoke<string>("check_local_shell", { path }).then(
        () => current && setFound((previous) => ({ ...previous, [path]: true })),
        () => current && setFound((previous) => ({ ...previous, [path]: false }))
      )
    }
    return () => {
      current = false
    }
  }, [shells])

  const entries = useMemo(() => buildShellEntries(detected, config, t), [config, detected, t])
  const isMissing = (shell: LocalShell) => found[shell.path.trim()] === false

  const save = useCallback((updates: Partial<AppConfig>) => saveSettings(updates), [saveSettings])

  return {
    entries,
    defaultKey: defaultShellKey(config),
    isMissing,
    makeDefault: (entry: ShellEntry) => save(defaultShellUpdates(entry.choice)),
    toggleHidden: (key: string) =>
      save({ hidden_shells: toggledHiddenShell(config.hidden_shells, key) }),
    saveShell: (shell: LocalShell, makeDefault: boolean) =>
      save({
        ...savedLocalShell(config, shell),
        ...(makeDefault
          ? defaultShellUpdates({
              shell: "custom",
              customPath: shell.path,
              customArgs: shell.args,
              shellId: shell.id,
            })
          : {}),
      }),
    removeShell: (id: string) => save(removedLocalShell(config, id)),
    moveShell: (id: string, offset: -1 | 1) =>
      save({ local_shells: movedLocalShell(shells, id, offset) }),
  }
}
