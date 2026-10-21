import React from "react"
import { Copy, Pencil, Plus, RotateCcw, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { type useHighlightSettings } from "@/components/SettingsDialog/useHighlightSettings"
import { useConfirmDialog, usePromptDialog } from "@/components/ui/app-dialog"
import { Button } from "@/components/ui/button"
import { Select } from "@/components/ui/select"
import {
  BUILTIN_HIGHLIGHT_SET_ID,
  type HighlightSet,
  highlightSetName,
  MAX_HIGHLIGHT_SETS,
} from "@/lib/keywordHighlight/config"
import {
  addHighlightSet,
  deleteHighlightSet,
  restoreBuiltinRules,
} from "@/lib/keywordHighlight/edit"

type HighlightSetToolbarProps = Pick<
  ReturnType<typeof useHighlightSettings>,
  "edit" | "editSet"
> & {
  sets: readonly HighlightSet[]
  editingSet: HighlightSet
  onEditingSetChange: (setId: string) => void
}

/** Picks the set being edited, and adds, copies, renames and deletes sets. */
export const HighlightSetToolbar: React.FC<HighlightSetToolbarProps> = ({
  sets,
  editingSet,
  edit,
  editSet,
  onEditingSetChange,
}) => {
  const { t } = useTranslation()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const { prompt, PromptDialog } = usePromptDialog()
  const editingName = highlightSetName(editingSet, t)
  const full = sets.length >= MAX_HIGHLIGHT_SETS

  const askName = async (title: string, defaultValue: string) => {
    const name = await prompt({
      title,
      label: t("keywordHighlight.setName"),
      defaultValue,
    })
    return name?.trim() || null
  }

  const addSet = async (source?: HighlightSet) => {
    const name = await askName(
      t(source ? "keywordHighlight.duplicateSet" : "keywordHighlight.newSet"),
      source
        ? t("keywordHighlight.copyName", { name: highlightSetName(source, t) })
        : t("keywordHighlight.newSetName", { count: sets.length + 1 })
    )
    if (!name) return
    let addedId = null as string | null
    edit((current) => {
      const added = addHighlightSet(current, name, source)
      if (!added) return current
      addedId = added.id
      return added.config
    })
    if (addedId) onEditingSetChange(addedId)
  }

  const renameSet = async () => {
    const name = await askName(t("keywordHighlight.renameSet"), editingName)
    if (name) editSet(editingSet.id, (set) => ({ ...set, name }))
  }

  const deleteSet = async () => {
    const confirmed = await confirm({
      title: t("keywordHighlight.deleteSetTitle", { name: editingName }),
      description: t("keywordHighlight.deleteSetDesc"),
      confirmText: t("keywordHighlight.deleteSet"),
      variant: "destructive",
    })
    if (!confirmed) return
    edit((current) => deleteHighlightSet(current, editingSet.id))
    const next = sets.find((set) => set.id !== editingSet.id)
    if (next) onEditingSetChange(next.id)
  }

  const restoreRules = async () => {
    const confirmed = await confirm({
      title: t("keywordHighlight.restoreBuiltinTitle"),
      description: t("keywordHighlight.restoreBuiltinDesc"),
      confirmText: t("keywordHighlight.restoreBuiltin"),
    })
    if (confirmed) editSet(editingSet.id, restoreBuiltinRules)
  }

  const iconButton = (
    label: string,
    icon: React.ReactNode,
    onClick: () => void,
    disabled = false
  ) => (
    <Button
      type="button"
      variant="outline"
      size="icon-sm"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
    >
      {icon}
    </Button>
  )

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Select
        aria-label={t("keywordHighlight.editingSet")}
        value={editingSet.id}
        className="w-48"
        onChange={(event) => onEditingSetChange(event.target.value)}
      >
        {sets.map((set) => (
          <option key={set.id} value={set.id}>
            {highlightSetName(set, t)}
          </option>
        ))}
      </Select>
      {iconButton(t("keywordHighlight.newSet"), <Plus />, () => void addSet(), full)}
      {iconButton(
        t("keywordHighlight.duplicateSet"),
        <Copy />,
        () => void addSet(editingSet),
        full
      )}
      {iconButton(t("keywordHighlight.renameSet"), <Pencil />, () => void renameSet())}
      {iconButton(
        t("keywordHighlight.deleteSet"),
        <Trash2 />,
        () => void deleteSet(),
        sets.length <= 1
      )}
      {editingSet.id === BUILTIN_HIGHLIGHT_SET_ID &&
        iconButton(t("keywordHighlight.restoreBuiltin"), <RotateCcw />, () => void restoreRules())}
      <ConfirmDialog />
      <PromptDialog />
    </div>
  )
}
