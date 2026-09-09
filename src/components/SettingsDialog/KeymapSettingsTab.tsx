import { AlertTriangle, Keyboard, RotateCcw, Trash2 } from "lucide-react"
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useConfig } from "@/contexts/ConfigContext"
import { useKeymap } from "@/contexts/KeymapContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { getActionsByGroup, KEYMAP_ACTION_GROUPS, type KeymapActionId } from "@/lib/keymap/actions"
import { eventToChord, formatChord, isMacPlatform, serializeChord } from "@/lib/keymap/chord"
import {
  findKeymapConflicts,
  getCliConflictHint,
  resolveEffectiveKeymap,
  type KeymapConfig,
} from "@/lib/keymap/keymap"
import { cn } from "@/lib/utils"

function actionLabelKey(actionId: KeymapActionId) {
  return `keymap.actions.${actionId}`
}

export const KeymapSettingsTab: React.FC = () => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { bindings, setDispatchSuppressed } = useKeymap()
  const { saveSettings } = useSettingsSave()
  const [recordingAction, setRecordingAction] = useState<KeymapActionId | null>(null)
  const [conflict, setConflict] = useState<{
    actionId: KeymapActionId
    owners: KeymapActionId[]
  } | null>(null)
  const [recordingError, setRecordingError] = useState<KeymapActionId | null>(null)
  const recorderRef = useRef<HTMLButtonElement>(null)
  const isMac = useMemo(() => isMacPlatform(), [])

  const persistKeymap = useCallback(
    async (nextKeymap: KeymapConfig, actionId: KeymapActionId) => {
      const conflicts = findKeymapConflicts(resolveEffectiveKeymap(nextKeymap))
      const firstConflict = conflicts.values().next().value as KeymapActionId[] | undefined
      if (firstConflict) {
        setConflict({ actionId, owners: firstConflict })
        return false
      }

      setConflict(null)
      return saveSettings({ keymap: nextKeymap })
    },
    [saveSettings]
  )

  const updateBinding = useCallback(
    async (actionId: KeymapActionId, chords: string[] | null | undefined) => {
      const nextBindings = { ...config.keymap.bindings }
      if (chords === undefined) delete nextBindings[actionId]
      else nextBindings[actionId] = chords
      return persistKeymap({ bindings: nextBindings }, actionId)
    },
    [config.keymap, persistKeymap]
  )

  useEffect(() => {
    if (!recordingAction) return

    setDispatchSuppressed(true)
    recorderRef.current?.focus()
    const handleRecord = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()

      if (event.key === "Escape") {
        setRecordingAction(null)
        setConflict(null)
        setRecordingError(null)
        return
      }
      if (event.key === "Backspace" || event.key === "Delete") {
        void updateBinding(recordingAction, null).then((saved) => {
          if (saved) setRecordingAction(null)
        })
        return
      }

      const chord = eventToChord(event, isMac)
      const serialized = chord ? serializeChord(chord) : null
      if (!serialized) return

      if (recordingAction === "tabs.switchToNth" && !/^[1-9]$/.test(chord!.key)) {
        setRecordingError(recordingAction)
        return
      }

      const nextChords =
        recordingAction === "tabs.switchToNth"
          ? Array.from({ length: 9 }, (_, index) =>
              serializeChord({ ...chord!, key: String(index + 1) })
            ).filter((value): value is string => value !== null)
          : [serialized]

      setRecordingError(null)
      void updateBinding(recordingAction, nextChords).then((saved) => {
        if (saved) setRecordingAction(null)
      })
    }

    window.addEventListener("keydown", handleRecord, true)
    return () => {
      window.removeEventListener("keydown", handleRecord, true)
      setDispatchSuppressed(false)
    }
  }, [isMac, recordingAction, setDispatchSuppressed, updateBinding])

  const handleResetOverrides = () => {
    setConflict(null)
    setRecordingError(null)
    void saveSettings({ keymap: { bindings: {} } })
  }

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Keyboard size={16} />}
          title={t("keymap.title")}
          description={t("keymap.description")}
        >
          <div className="flex justify-end">
            <Button type="button" variant="outline" onClick={handleResetOverrides}>
              <RotateCcw aria-hidden="true" />
              {t("keymap.resetOverrides")}
            </Button>
          </div>
        </SettingsSection>

        {KEYMAP_ACTION_GROUPS.map((group) => (
          <SettingsSection key={group} title={t(`keymap.groups.${group}`)}>
            <div className="border-border divide-border overflow-hidden rounded-md border">
              {getActionsByGroup(group).map((action) => {
                const chords = bindings[action.id] ?? []
                const isRecording = recordingAction === action.id
                const actionConflict = conflict?.actionId === action.id ? conflict : null
                const actionRecordingError = recordingError === action.id
                const cliHint = chords
                  .map((chord) => getCliConflictHint(chord, isMac))
                  .find(Boolean)

                return (
                  <div
                    key={action.id}
                    className={cn(
                      "bg-card flex flex-col gap-3 border-b p-3 last:border-b-0 sm:flex-row sm:items-center sm:justify-between",
                      actionConflict && "bg-destructive/5"
                    )}
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-medium">{t(actionLabelKey(action.id))}</div>
                      {actionConflict ? (
                        <p className="text-destructive mt-1 text-xs" role="alert">
                          {t("keymap.conflictDescription", {
                            actions: actionConflict.owners
                              .map((owner) => t(actionLabelKey(owner)))
                              .join(", "),
                          })}
                        </p>
                      ) : actionRecordingError ? (
                        <p className="text-destructive mt-1 text-xs" role="alert">
                          {t("keymap.numberRequired")}
                        </p>
                      ) : cliHint ? (
                        <p className="mt-1 flex items-start gap-1 text-xs text-amber-600 dark:text-amber-400">
                          <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden="true" />
                          {t(`keymap.cliHints.${cliHint}`)}
                        </p>
                      ) : null}
                    </div>

                    <div className="flex min-w-0 items-center gap-2 sm:max-w-[60%] sm:justify-end">
                      <button
                        ref={isRecording ? recorderRef : undefined}
                        type="button"
                        className={cn(
                          "border-input bg-background focus-visible:ring-ring/50 hover:bg-accent flex min-h-8 min-w-28 cursor-pointer flex-wrap items-center justify-center gap-1 rounded-md border px-2 py-1 text-xs transition-colors focus-visible:ring-[3px] focus-visible:outline-none",
                          isRecording && "border-primary text-primary ring-primary/20 ring-2"
                        )}
                        aria-label={t("keymap.recordFor", { action: t(actionLabelKey(action.id)) })}
                        aria-pressed={isRecording}
                        onClick={() => {
                          setConflict(null)
                          setRecordingError(null)
                          setRecordingAction(isRecording ? null : action.id)
                        }}
                      >
                        {isRecording ? (
                          <span className="text-primary">{t("keymap.recording")}</span>
                        ) : chords.length > 0 ? (
                          chords.map((chord) => (
                            <kbd
                              key={chord}
                              className="border-border bg-muted rounded border px-1.5 py-0.5 font-mono text-[11px] whitespace-nowrap"
                            >
                              {formatChord(chord, isMac)}
                            </kbd>
                          ))
                        ) : (
                          <span className="text-muted-foreground">{t("keymap.unbound")}</span>
                        )}
                      </button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        title={t("keymap.clear")}
                        aria-label={t("keymap.clear")}
                        onClick={() => void updateBinding(action.id, null)}
                      >
                        <Trash2 aria-hidden="true" />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-sm"
                        title={t("keymap.restoreDefault")}
                        aria-label={t("keymap.restoreDefault")}
                        disabled={config.keymap.bindings[action.id] === undefined}
                        onClick={() => void updateBinding(action.id, undefined)}
                      >
                        <RotateCcw aria-hidden="true" />
                      </Button>
                    </div>
                  </div>
                )
              })}
            </div>
          </SettingsSection>
        ))}

        <p className="text-muted-foreground pb-2 text-xs" aria-live="polite">
          {recordingAction ? t("keymap.recordingHelp") : t("keymap.help")}
        </p>
      </div>
    </ScrollArea>
  )
}
