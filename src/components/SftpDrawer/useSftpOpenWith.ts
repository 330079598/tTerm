import { useCallback, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import type { AppChoice, OpenIn, OpenWithChoices } from "@/components/SftpDrawer/externalEdit"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import { useConfig } from "@/contexts/ConfigContext"
import { pickEditorProgram } from "@/lib/externalEditor"

interface UseSftpOpenWithOptions {
  openEntry: (entry: SftpDirectoryEntry, openIn: OpenIn) => Promise<void>
}

/** The Open With list on screen: the file and the applications for it. */
export interface OpenWithState {
  entry: SftpDirectoryEntry
  apps: AppChoice[]
}

/**
 * Open With for a remote file, the way the system offers it: Windows shows
 * its own dialog, macOS lists the applications for the file's kind here,
 * and elsewhere the user picks a program file.
 */
export function useSftpOpenWith({ openEntry }: UseSftpOpenWithOptions) {
  const { t } = useTranslation()
  const { saveConfig } = useConfig()
  const [state, setState] = useState<OpenWithState | null>(null)

  const pickProgram = useCallback(
    async (entry: SftpDirectoryEntry) => {
      const program = await pickEditorProgram(t("sftp.openWith.chooseProgram"))
      if (program) await openEntry(entry, { kind: "program", program })
    },
    [openEntry, t]
  )

  // Icons not drawn before arrive after the list shows.
  const fillIcons = useCallback(async (entry: SftpDirectoryEntry, paths: string[]) => {
    const icons: Record<string, string | null> = await invoke<Record<string, string | null>>(
      "sftp_external_app_icons",
      {
        paths,
      }
    ).catch((error: unknown) => {
      console.error(error)
      return {}
    })
    setState((current) =>
      current?.entry === entry
        ? {
            entry,
            apps: current.apps.map((app) => ({ ...app, icon: app.icon ?? icons[app.path] })),
          }
        : current
    )
  }, [])

  const start = useCallback(
    async (entry: SftpDirectoryEntry) => {
      const choices: OpenWithChoices = await invoke<OpenWithChoices>(
        "sftp_external_open_with_choices",
        { fileName: entry.name }
      ).catch((error: unknown) => {
        console.error(error)
        return { systemChooser: false }
      })
      const apps = choices.apps ?? []
      if (choices.systemChooser) await openEntry(entry, { kind: "chooser" })
      else if (apps.length > 0) {
        setState({ entry, apps })
        const missing = apps.filter((app) => !app.icon).map((app) => app.path)
        if (missing.length > 0) void fillIcons(entry, missing)
      } else await pickProgram(entry)
    },
    [fillIcons, openEntry, pickProgram]
  )

  const close = useCallback(() => setState(null), [])

  const pick = useCallback(
    async (app: AppChoice, asEditor: boolean) => {
      if (!state) return
      setState(null)
      if (asEditor) await saveConfig({ sftp_external_editor: app.path }).catch(console.error)
      await openEntry(state.entry, { kind: "program", program: app.path })
    },
    [openEntry, saveConfig, state]
  )

  const other = useCallback(async () => {
    if (!state) return
    setState(null)
    await pickProgram(state.entry)
  }, [pickProgram, state])

  return { start, dialogProps: { state, onClose: close, onPick: pick, onOther: other } }
}
