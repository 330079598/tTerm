import React from "react"
import { useDraggable, useDroppable } from "@dnd-kit/react"
import { GripVertical, Pencil, PlugZap, Server, Terminal, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  profileDragId,
  profileDropId,
  buildProfileSubtitle,
} from "@/components/SettingsDialog/profileGroupDnd"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { buildConnectionFromProfile } from "@/lib/profileConnections"
import { cn } from "@/lib/utils"
import type { SavedProfile, Tab } from "@/types/tab"

interface ProfileDragRowProps {
  profile: SavedProfile
  isSelected: boolean
  onConnectProfile?: (connection: Omit<Tab, "id" | "isActive">) => void
  onDeleteProfile: (profile: SavedProfile) => void
  onEditProfile?: (profile: SavedProfile) => void
  onToggleSelected: (profileId: string) => void
}

export const ProfileDragRow: React.FC<ProfileDragRowProps> = ({
  profile,
  isSelected,
  onConnectProfile,
  onDeleteProfile,
  onEditProfile,
  onToggleSelected,
}) => {
  const { t } = useTranslation()
  const { ref: draggableRef, isDragging } = useDraggable({ id: profileDragId(profile.id) })
  const { ref: droppableRef, isDropTarget } = useDroppable({ id: profileDropId(profile.id) })
  const isTerminal = profile.connection_type === "terminal"
  const Icon = isTerminal ? Terminal : Server
  const hasProfileActions = Boolean(onConnectProfile || onEditProfile || onDeleteProfile)
  const setNodeRef = (node: HTMLDivElement | null) => {
    draggableRef(node)
    droppableRef(node)
  }

  return (
    <div
      ref={setNodeRef}
      className={cn(
        "group border-border/70 bg-background/70 hover:bg-muted/35 focus-visible:ring-ring/50 flex cursor-grab items-start gap-3 rounded-md border px-3 py-2.5 text-left transition-colors outline-none focus-visible:ring-[3px]",
        isDragging && "cursor-grabbing opacity-60",
        isDropTarget && "border-primary/70 bg-primary/5",
        isSelected && "border-primary/70 bg-primary/5"
      )}
      role="listitem"
      tabIndex={0}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <Checkbox
            checked={isSelected}
            onCheckedChange={() => onToggleSelected(profile.id)}
            onClick={(event) => event.stopPropagation()}
            aria-label={t("profiles.selectConnection", { name: profile.name })}
            className="mt-0.5 shrink-0"
          />
        </TooltipTrigger>
        <TooltipContent>{t("profiles.selectConnection", { name: profile.name })}</TooltipContent>
      </Tooltip>
      <GripVertical size={15} className="text-muted-foreground mt-0.5 shrink-0" />
      <Icon size={15} className="text-muted-foreground mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-2">
          <div className="truncate text-sm font-medium">{profile.name}</div>
          <Badge variant="outline" className="h-5 shrink-0 px-1.5 text-[10px]">
            {isTerminal ? t("profiles.localBadge") : "SSH"}
          </Badge>
        </div>
        <div className="text-muted-foreground mt-1 truncate text-xs">
          {buildProfileSubtitle(profile, t)}
        </div>
      </div>
      {hasProfileActions && (
        <div className="flex shrink-0 items-center gap-1 self-center opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
          {onConnectProfile && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t("profiles.connect")}
                  onClick={(event) => {
                    event.stopPropagation()
                    onConnectProfile(buildConnectionFromProfile(profile))
                  }}
                >
                  <PlugZap size={13} />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("profiles.connect")}</TooltipContent>
            </Tooltip>
          )}
          {onEditProfile && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t("profiles.edit")}
                  onClick={(event) => {
                    event.stopPropagation()
                    onEditProfile(profile)
                  }}
                >
                  <Pencil size={13} />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t("profiles.edit")}</TooltipContent>
            </Tooltip>
          )}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="hover:text-destructive"
                aria-label={t("profiles.delete")}
                onClick={(event) => {
                  event.stopPropagation()
                  onDeleteProfile(profile)
                }}
              >
                <Trash2 size={13} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("profiles.delete")}</TooltipContent>
          </Tooltip>
        </div>
      )}
    </div>
  )
}
