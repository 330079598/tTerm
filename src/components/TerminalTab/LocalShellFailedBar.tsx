import { useState } from "react"
import { CircleAlert } from "lucide-react"
import { useTranslation } from "react-i18next"

import { LocalShellEditor } from "@/components/SettingsDialog/LocalShells/LocalShellEditor"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import {
  customShellKey,
  defaultShellChoice,
  defaultShellKey,
  ownShellChoice,
  savedLocalShellUpdates,
  startsShell,
  type LocalShellChoice,
} from "@/lib/localShells"

interface LocalShellFailedBarProps {
  connection: TerminalTabProps["connection"]
  /** What starting the shell answered. */
  error: string
  onRetry: () => void
  onRestartWithShell: (choice: LocalShellChoice) => void
}

/**
 * Shown when a local tab's shell didn't start: fix the user's shell where it
 * failed, try again, or open the default shell in this tab instead.
 */
export function LocalShellFailedBar({
  connection,
  error,
  onRetry,
  onRestartWithShell,
}: LocalShellFailedBarProps) {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [editing, setEditing] = useState(false)
  const id = connection?.terminalShellId
  const own = id ? config.local_shells.find((shell) => shell.id === id) : undefined
  const fallback = defaultShellChoice(config)
  const isDefault = startsShell(connection, fallback)

  return (
    <>
      <div
        role="alert"
        className="bg-card border-destructive/40 absolute top-3 left-1/2 z-20 flex w-[min(560px,calc(100%-24px))] -translate-x-1/2 flex-wrap items-center gap-x-3 gap-y-2 rounded-md border p-2 pl-3 text-xs shadow-lg"
      >
        <div className="flex min-w-0 flex-[1_1_16em] items-center gap-2.5">
          <CircleAlert size={16} className="text-destructive shrink-0" aria-hidden="true" />
          <div className="min-w-0">
            <div className="font-semibold">{t("localShells.startFailed")}</div>
            <div className="text-muted-foreground truncate" title={error}>
              {error}
            </div>
          </div>
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-2">
          {own && (
            <Button type="button" size="xs" variant="outline" onClick={() => setEditing(true)}>
              {t("localShells.editThis")}
            </Button>
          )}
          <Button type="button" size="xs" variant="outline" onClick={onRetry}>
            {t("localShells.retry")}
          </Button>
          {!isDefault && (
            <Button type="button" size="xs" onClick={() => onRestartWithShell(fallback)}>
              {t("localShells.openDefault")}
            </Button>
          )}
        </div>
      </div>

      {own && (
        <Dialog open={editing} onOpenChange={setEditing}>
          <DialogContent className="sm:max-w-lg">
            <DialogHeader>
              <DialogTitle>
                {t("localShells.editTitle", { name: own.name || own.path })}
              </DialogTitle>
            </DialogHeader>
            <LocalShellEditor
              shell={own}
              isDefault={defaultShellKey(config) === customShellKey(own.id)}
              onCancel={() => setEditing(false)}
              onSave={(saved, makeDefault) => {
                void saveSettings(savedLocalShellUpdates(config, saved, makeDefault)).then((ok) => {
                  if (!ok) return
                  setEditing(false)
                  onRestartWithShell(ownShellChoice(saved))
                })
              }}
            />
          </DialogContent>
        </Dialog>
      )}
    </>
  )
}
