import React from "react"
import { useDraggable, useDroppable } from "@dnd-kit/react"
import {
  Check,
  ChevronDown,
  ChevronRight,
  FolderPlus,
  FolderTree,
  GripVertical,
  Pencil,
  Trash2,
  X,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  UNGROUPED_KEY,
  groupBodyDropId,
  groupDragId,
  groupDropId,
  groupListId,
  ROOT_DROP_ID,
} from "@/components/SettingsDialog/profileGroupDnd"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { isImeKeyEvent } from "@/lib/ime"
import { cn } from "@/lib/utils"
import type { ProfileGroup } from "@/types/tab"

interface ProfileGroupCardProps {
  children: React.ReactNode
  /** The saved connections in the group and the groups inside it. */
  count: number
  /** The group; none for the card of ungrouped connections. */
  group?: ProfileGroup
  groupBusy: boolean
  isCollapsed: boolean
  /**
   * Refuses drops while this group, or one it sits in, is dragged: the card
   * moves with the pointer, so its own heading would otherwise catch the drop.
   */
  dropDisabled?: boolean
  onToggleCollapsed: (groupKey: string) => void
  onAddSubgroup?: (name: string) => Promise<boolean>
  onRename?: (name: string) => Promise<boolean>
  onDelete?: () => void
}

const IconAction: React.FC<{
  label: string
  onClick: () => void
  disabled?: boolean
  className?: string
  children: React.ReactNode
}> = ({ label, onClick, disabled, className, children }) => (
  <Tooltip>
    <TooltipTrigger asChild>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        className={className}
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
      >
        {children}
      </Button>
    </TooltipTrigger>
    <TooltipContent>{label}</TooltipContent>
  </Tooltip>
)

/** A one-line name field that saves on Enter and gives up on Escape. */
const NameField: React.FC<{
  value: string
  onChange: (value: string) => void
  onSave: () => void
  onCancel: () => void
  disabled: boolean
  placeholder?: string
}> = ({ value, onChange, onSave, onCancel, disabled, placeholder }) => (
  <Input
    value={value}
    onChange={(event) => onChange(event.target.value)}
    onKeyDown={(event) => {
      if (isImeKeyEvent(event.nativeEvent)) return
      if (event.key === "Enter") {
        event.preventDefault()
        onSave()
      }
      if (event.key === "Escape") onCancel()
    }}
    disabled={disabled}
    placeholder={placeholder}
    className="h-8"
    autoFocus
  />
)

/** Where a dragged connection or group lands in a group that holds nothing. */
export const GroupDropHint: React.FC<{ groupKey: string; disabled?: boolean }> = ({
  groupKey,
  disabled,
}) => {
  const { t } = useTranslation()
  const { ref, isDropTarget } = useDroppable({ id: groupBodyDropId(groupKey), disabled })
  return (
    <div
      ref={ref}
      className={cn(
        "text-muted-foreground flex min-h-16 items-center justify-center rounded-md border border-dashed px-4 text-center text-xs transition-colors",
        isDropTarget && "border-primary/70 bg-primary/5"
      )}
    >
      {t("profileGroups.dropEmpty", { defaultValue: "Drop saved connections here." })}
    </div>
  )
}

/**
 * Where a dragged group is dropped to move it to the top. It covers the bar
 * above the tree rather than pushing the tree down, which would move the
 * groups out from under the pointer as the drag starts.
 */
export const RootDropZone: React.FC = () => {
  const { t } = useTranslation()
  const { ref, isDropTarget } = useDroppable({ id: ROOT_DROP_ID })
  return (
    <div
      ref={ref}
      className={cn(
        "bg-card text-muted-foreground absolute inset-0 flex items-center justify-center rounded-md border border-dashed px-4 text-center text-xs transition-colors",
        isDropTarget && "border-primary/70 text-foreground"
      )}
    >
      {t("profileGroups.dropToTop", { defaultValue: "Drop here to move the group to the top" })}
    </div>
  )
}

export const ProfileGroupCard: React.FC<ProfileGroupCardProps> = ({
  children,
  count,
  group,
  groupBusy,
  isCollapsed,
  dropDisabled,
  onToggleCollapsed,
  onAddSubgroup,
  onRename,
  onDelete,
}) => {
  const { t } = useTranslation()
  const groupKey = group?.id ?? UNGROUPED_KEY
  const { ref: dropRef, isDropTarget } = useDroppable({
    id: groupDropId(groupKey),
    disabled: dropDisabled,
  })
  const [renameDraft, setRenameDraft] = React.useState<string | null>(null)
  // The grip and the name drag the group. The buttons sit outside that
  // handle: a press inside a handle starts a drag at once, even on a button.
  const {
    ref: dragRef,
    handleRef,
    isDragging,
  } = useDraggable({
    id: groupDragId(groupKey),
    disabled: !group || renameDraft !== null,
  })
  const [subgroupDraft, setSubgroupDraft] = React.useState<string | null>(null)
  const displayName = group?.name ?? t("profiles.ungrouped")
  const listId = groupListId(groupKey)
  const toggleLabel = isCollapsed
    ? t("profiles.expandGroup", { group: displayName })
    : t("profiles.collapseGroup", { group: displayName })

  const saveRename = async () => {
    if (renameDraft === null || !onRename) return
    if (await onRename(renameDraft)) setRenameDraft(null)
  }
  const saveSubgroup = async () => {
    if (!subgroupDraft?.trim() || !onAddSubgroup) return
    if (await onAddSubgroup(subgroupDraft)) setSubgroupDraft(null)
  }
  const startSubgroup = () => {
    setSubgroupDraft("")
    if (isCollapsed) onToggleCollapsed(groupKey)
  }

  return (
    <section
      ref={dragRef}
      className={cn(
        "border-border/70 bg-card flex flex-col rounded-md border transition-colors",
        isDragging && "opacity-60"
      )}
    >
      <div
        ref={dropRef}
        className={cn(
          "border-border/70 flex items-center gap-2 rounded-t-md border-b px-3 py-2.5 transition-colors",
          isCollapsed && "rounded-b-md border-b-transparent",
          isDropTarget && "bg-primary/5 ring-primary/70 ring-1"
        )}
      >
        <IconAction label={toggleLabel} onClick={() => onToggleCollapsed(groupKey)}>
          {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
        </IconAction>
        <div
          ref={handleRef}
          className={cn(
            "flex min-w-0 flex-1 items-center gap-2",
            group && renameDraft === null && "cursor-grab active:cursor-grabbing"
          )}
        >
          {group ? (
            <span
              className="text-muted-foreground flex size-7 shrink-0 items-center justify-center rounded-md border"
              aria-label={t("profileGroups.dragGroup", {
                name: group.name,
                defaultValue: "Drag {{name}} into another group",
              })}
            >
              <GripVertical size={14} />
            </span>
          ) : (
            <span className="bg-muted text-muted-foreground flex size-7 shrink-0 items-center justify-center rounded-md border">
              <FolderTree size={14} />
            </span>
          )}
          <div className="min-w-0 flex-1">
            {renameDraft !== null ? (
              <NameField
                value={renameDraft}
                onChange={setRenameDraft}
                onSave={saveRename}
                onCancel={() => setRenameDraft(null)}
                disabled={groupBusy}
              />
            ) : (
              <div className="flex min-w-0 items-baseline gap-2">
                <span className="truncate text-sm font-semibold">{displayName}</span>
                <span className="text-muted-foreground shrink-0 text-xs">
                  {t("profileGroups.profileCount", {
                    count,
                    defaultValue: "{{count}} saved connections",
                  })}
                </span>
              </div>
            )}
          </div>
        </div>

        {group && (
          <div className="flex shrink-0 items-center gap-1">
            {renameDraft !== null ? (
              <>
                <IconAction
                  label={t("common.save")}
                  onClick={saveRename}
                  disabled={groupBusy || !renameDraft.trim()}
                >
                  <Check size={14} />
                </IconAction>
                <IconAction
                  label={t("common.cancel")}
                  onClick={() => setRenameDraft(null)}
                  disabled={groupBusy}
                >
                  <X size={14} />
                </IconAction>
              </>
            ) : (
              <>
                <IconAction
                  label={t("profileGroups.addSubgroup", { defaultValue: "New subgroup" })}
                  onClick={startSubgroup}
                  disabled={groupBusy}
                >
                  <FolderPlus size={14} />
                </IconAction>
                <IconAction
                  label={t("profileGroups.rename")}
                  onClick={() => setRenameDraft(group.name)}
                  disabled={groupBusy}
                >
                  <Pencil size={14} />
                </IconAction>
                <IconAction
                  label={t("profileGroups.delete")}
                  onClick={() => onDelete?.()}
                  disabled={groupBusy}
                  className="hover:text-destructive"
                >
                  <Trash2 size={14} />
                </IconAction>
              </>
            )}
          </div>
        )}
      </div>

      {!isCollapsed && (
        <div id={listId} className="flex flex-col gap-2 p-3">
          {subgroupDraft !== null && (
            <div className="flex items-center gap-2">
              <NameField
                value={subgroupDraft}
                onChange={setSubgroupDraft}
                onSave={saveSubgroup}
                onCancel={() => setSubgroupDraft(null)}
                disabled={groupBusy}
                placeholder={t("profileGroups.subgroupPlaceholder", {
                  defaultValue: "Subgroup name",
                })}
              />
              <Button
                type="button"
                size="sm"
                onClick={saveSubgroup}
                disabled={groupBusy || !subgroupDraft.trim()}
              >
                {t("profileGroups.addButton")}
              </Button>
            </div>
          )}
          {children}
        </div>
      )}
    </section>
  )
}
