import { Fragment, useState } from "react"
import { Plus, SquareTerminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { LocalShellEditor } from "@/components/SettingsDialog/LocalShells/LocalShellEditor"
import { LocalShellRow } from "@/components/SettingsDialog/LocalShells/LocalShellRow"
import { useLocalShellSettings } from "@/components/SettingsDialog/LocalShells/useLocalShellSettings"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { useConfig } from "@/contexts/ConfigContext"
import type { LocalShell } from "@/lib/appConfig"
import { customShellKey, newLocalShellId } from "@/lib/localShells"
import { getDetectedPlatform } from "@/lib/uiPlatform"

/**
 * The shells local tabs can start: the ones found on this machine and the
 * ones the user added, which of them is the default, and which show in the
 * new tab list.
 */
export function LocalShellSettings() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const shells = useLocalShellSettings()
  // The shell being added or edited; a new one isn't in the list yet.
  const [editing, setEditing] = useState<LocalShell | null>(null)
  const ownShells = config.local_shells
  const adding = editing !== null && !ownShells.some((shell) => shell.id === editing.id)
  const autoDetail =
    getDetectedPlatform() === "windows"
      ? t("localShells.autoDetailWindows")
      : t("localShells.autoDetailUnix")

  const startAdding = (path = "", name = "") =>
    setEditing({ id: newLocalShellId(), name, path, args: "" })

  const editor = (shell: LocalShell) => (
    <LocalShellEditor
      key={shell.id}
      shell={shell}
      isDefault={shells.defaultKey === customShellKey(shell.id)}
      onCancel={() => setEditing(null)}
      onSave={(saved, makeDefault) => {
        void shells.saveShell(saved, makeDefault).then((ok) => {
          if (ok) setEditing(null)
        })
      }}
    />
  )

  return (
    <SettingsSection
      icon={<SquareTerminal size={16} />}
      title={t("localShells.title")}
      description={t("localShells.description")}
    >
      <SettingsRow
        title={t("localShells.listTitle")}
        description={t("localShells.listDescription")}
        action={
          <Button type="button" size="sm" variant="outline" onClick={() => startAdding()}>
            <Plus size={14} />
            {t("localShells.add")}
          </Button>
        }
      >
        {adding && editing && <div className="mb-3">{editor(editing)}</div>}
        <div className="divide-border divide-y">
          {shells.entries.map((entry) => {
            const id = entry.choice.shellId
            const own = id ? ownShells.find((shell) => shell.id === id) : undefined
            const index = own ? ownShells.indexOf(own) : -1
            return (
              <Fragment key={entry.key}>
                <LocalShellRow
                  entry={entry}
                  isDefault={entry.key === shells.defaultKey}
                  missing={own ? shells.isMissing(own) : false}
                  canMoveUp={index > 0}
                  canMoveDown={index >= 0 && index < ownShells.length - 1}
                  autoDetail={autoDetail}
                  onMakeDefault={() => void shells.makeDefault(entry)}
                  onToggleHidden={() => void shells.toggleHidden(entry.key)}
                  onCopy={() => startAdding(entry.detail, entry.label)}
                  onEdit={() => setEditing(own ?? null)}
                  onRemove={() => {
                    if (id) void shells.removeShell(id)
                  }}
                  onMove={(offset) => {
                    if (id) void shells.moveShell(id, offset)
                  }}
                />
                {own && editing?.id === own.id && <div className="pb-3">{editor(own)}</div>}
              </Fragment>
            )
          })}
        </div>
      </SettingsRow>
    </SettingsSection>
  )
}
