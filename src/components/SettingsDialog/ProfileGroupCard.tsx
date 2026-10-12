import React from "react"
import { useDroppable } from "@dnd-kit/react"
import { Check, ChevronDown, ChevronRight, FolderTree, Pencil, Trash2, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  UNGROUPED_KEY,
  groupDropId,
  groupListId,
  type GroupColumn,
} from "@/components/SettingsDialog/profileGroupDnd"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { isImeKeyEvent } from "@/lib/ime"
import { cn } from "@/lib/utils"

interface ProfileGroupCardProps {
  children: React.ReactNode
  count: number
  group: GroupColumn
  groupBusy: boolean
  isCollapsed: boolean
  isEditing: boolean
  editingGroupDraft: string
  onCancelEdit: () => void
  onDeleteGroup: (name: string) => void
  onRenameGroup: (oldName: string) => void
  onStartEdit: (name: string) => void
  onToggleCollapsed: (groupKey: string) => void
  setEditingGroupDraft: React.Dispatch<React.SetStateAction<string>>
}

export const ProfileGroupCard: React.FC<ProfileGroupCardProps> = ({
  children,
  count,
  group,
  groupBusy,
  isCollapsed,
  isEditing,
  editingGroupDraft,
  onCancelEdit,
  onDeleteGroup,
  onRenameGroup,
  onStartEdit,
  onToggleCollapsed,
  setEditingGroupDraft,
}) => {
  const { t } = useTranslation()
  const { ref, isDropTarget } = useDroppable({ id: groupDropId(group.key) })
  const isUngrouped = group.key === UNGROUPED_KEY
  const displayName = isUngrouped ? t("profiles.ungrouped") : group.name
  const listId = groupListId(group.key)
  const toggleLabel = isCollapsed
    ? t("profiles.expandGroup", { group: displayName })
    : t("profiles.collapseGroup", { group: displayName })

  return (
    <section
      ref={ref}
      className={cn(
        "border-border/70 bg-card flex flex-col rounded-md border transition-colors",
        isCollapsed ? "min-h-0" : "min-h-[220px]",
        isDropTarget && "border-primary/70 bg-primary/5"
      )}
    >
      <div className="border-border/70 flex items-start gap-3 border-b px-4 py-3">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="-ml-1 shrink-0"
              onClick={() => onToggleCollapsed(group.key)}
              aria-expanded={!isCollapsed}
              aria-controls={listId}
              aria-label={toggleLabel}
            >
              {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{toggleLabel}</TooltipContent>
        </Tooltip>
        <div className="bg-muted text-muted-foreground flex size-8 shrink-0 items-center justify-center rounded-md border">
          <FolderTree size={15} />
        </div>
        <div className="min-w-0 flex-1">
          {isEditing ? (
            <Input
              value={editingGroupDraft}
              onChange={(event) => setEditingGroupDraft(event.target.value)}
              onKeyDown={(event) => {
                if (isImeKeyEvent(event.nativeEvent)) return
                if (event.key === "Enter") {
                  event.preventDefault()
                  onRenameGroup(group.name)
                }
                if (event.key === "Escape") {
                  onCancelEdit()
                }
              }}
              disabled={groupBusy}
              className="h-8"
              autoFocus
            />
          ) : (
            <>
              <div className="truncate text-sm font-semibold">{displayName}</div>
              <div className="text-muted-foreground mt-1 text-xs">
                {t("profileGroups.profileCount", {
                  count,
                  defaultValue: "{{count}} saved connections",
                })}
              </div>
            </>
          )}
        </div>

        {!isUngrouped && (
          <div className="flex shrink-0 items-center gap-1">
            {isEditing ? (
              <>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={() => onRenameGroup(group.name)}
                      disabled={groupBusy || !editingGroupDraft.trim()}
                      aria-label={t("common.save")}
                    >
                      <Check size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("common.save")}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={onCancelEdit}
                      disabled={groupBusy}
                      aria-label={t("common.cancel")}
                    >
                      <X size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("common.cancel")}</TooltipContent>
                </Tooltip>
              </>
            ) : (
              <>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={() => onStartEdit(group.name)}
                      disabled={groupBusy}
                      aria-label={t("profileGroups.rename")}
                    >
                      <Pencil size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("profileGroups.rename")}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="hover:text-destructive"
                      onClick={() => onDeleteGroup(group.name)}
                      disabled={groupBusy}
                      aria-label={t("profileGroups.delete")}
                    >
                      <Trash2 size={14} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("profileGroups.delete")}</TooltipContent>
                </Tooltip>
              </>
            )}
          </div>
        )}
      </div>

      {!isCollapsed && (
        <div id={listId} className="flex flex-1 flex-col gap-2 p-3">
          {children}
        </div>
      )}
    </section>
  )
}
