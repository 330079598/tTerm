import React from "react"
import { ChevronRight, Copy, Pencil, PlugZap, Server, Terminal, Trash2 } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import type { ConnectionType, SavedProfile } from "@/types/tab"

const connectionTypeIcons = {
  ssh: Server,
  terminal: Terminal,
} as const

const buildConnectionSubtitle = (
  profile: SavedProfile,
  t: (key: string, options?: Record<string, unknown>) => string
) => {
  if (profile.connection_type === "terminal") {
    return t("profiles.localTerminal")
  }

  if (!profile.host) {
    return t("profiles.connectionDetailsUnavailable")
  }

  return `${profile.username ? `${profile.username}@` : ""}${profile.host}${profile.port && profile.port !== 22 ? `:${profile.port}` : ""}`
}

const buildMetaItems = (
  profile: SavedProfile,
  t: (key: string, options?: Record<string, unknown>) => string
) => {
  if (profile.connection_type === "terminal") {
    return [t("profiles.localTerminal")]
  }

  const items = ["SSH"]

  if (profile.auth_method === "key") {
    items.push(t("profiles.authMethodKey"))
  } else if (profile.auth_method === "agent") {
    items.push(t("profiles.authMethodAgent"))
  } else if (profile.auth_method === "interactive") {
    items.push(t("profiles.authMethodInteractive"))
  } else if (profile.auth_method === "password") {
    items.push(t("profiles.authMethodPassword"))
  }

  if (profile.port && profile.port !== 22) {
    items.push(t("profiles.portDisplay", { port: profile.port }))
  }

  const jumpHosts = profile.jump_hosts ?? []

  const useJumpHost = profile.use_jump_host ?? jumpHosts.length > 0
  if (useJumpHost && jumpHosts.length === 1) {
    items.push(t("jumpHost.via", { host: jumpHosts[0].host }))
  } else if (useJumpHost && jumpHosts.length > 1) {
    items.push(t("jumpHost.viaCount", { count: jumpHosts.length }))
  }

  return items
}

export const ProfileRow: React.FC<{
  profile: SavedProfile
  isActive: boolean
  isDeleting: boolean
  onConnect: (p: SavedProfile) => void
  onEdit: (p: SavedProfile) => void
  onDuplicate: (p: SavedProfile) => void
  onDelete: (id: string) => void
  onFocusRow: (id: string) => void
  rowRef: (node: HTMLDivElement | null) => void
  t: (key: string, options?: Record<string, unknown>) => string
}> = ({
  profile,
  isActive,
  isDeleting,
  onConnect,
  onEdit,
  onDuplicate,
  onDelete,
  onFocusRow,
  rowRef,
  t,
}) => {
  const connectionType = profile.connection_type as ConnectionType
  const Icon = connectionTypeIcons[connectionType] ?? Server
  const subtitle = buildConnectionSubtitle(profile, t)
  const metaItems = buildMetaItems(profile, t)
  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter") {
      event.preventDefault()
      onConnect(profile)
      return
    }

    if (event.key === " ") {
      event.preventDefault()
      onFocusRow(profile.id)
    }
  }

  return (
    <div
      ref={rowRef}
      role="option"
      tabIndex={0}
      aria-selected={isActive}
      onClick={() => onFocusRow(profile.id)}
      onDoubleClick={() => onConnect(profile)}
      onKeyDown={handleKeyDown}
      className={cn(
        "group focus-visible:ring-ring/50 relative flex w-full items-start gap-3 rounded-xl border px-3 py-3 text-left transition-colors outline-none focus-visible:ring-[3px]",
        isActive
          ? "border-border bg-muted/55 shadow-[inset_0_0_0_1px_hsl(var(--border)/0.45)]"
          : "border-border/60 hover:bg-muted/30"
      )}
    >
      {isActive && (
        <div className="bg-foreground/28 absolute inset-y-2 left-0 w-0.5 rounded-full" />
      )}
      <div className="bg-muted text-muted-foreground flex size-9 shrink-0 items-center justify-center rounded-lg border">
        <Icon size={16} />
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-2">
          <div className="truncate text-sm font-medium tracking-[-0.01em]">{profile.name}</div>
          <Badge variant="outline" className="hidden shrink-0 sm:inline-flex">
            {profile.connection_type === "terminal" ? t("profiles.localBadge") : "SSH"}
          </Badge>
        </div>

        <div className="text-muted-foreground mt-1 truncate text-sm">{subtitle}</div>

        <div className="text-muted-foreground mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
          {metaItems.map((item, index) => (
            <React.Fragment key={`${profile.id}-${item}`}>
              {index > 0 && <span className="text-border">•</span>}
              <span>{item}</span>
            </React.Fragment>
          ))}
        </div>
      </div>

      <div
        className={cn(
          "flex shrink-0 items-center gap-1 self-center transition-opacity",
          isActive
            ? "opacity-100"
            : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
        )}
      >
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={t("profiles.connect")}
              onClick={(event) => {
                event.stopPropagation()
                onConnect(profile)
              }}
            >
              <PlugZap size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.connect")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={t("profiles.edit")}
              onClick={(event) => {
                event.stopPropagation()
                onEdit(profile)
              }}
            >
              <Pencil size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.edit")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={t("profiles.copy")}
              onClick={(event) => {
                event.stopPropagation()
                onDuplicate(profile)
              }}
            >
              <Copy size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.copy")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="hover:text-destructive"
              disabled={isDeleting}
              aria-label={t("profiles.delete")}
              onClick={(event) => {
                event.stopPropagation()
                onDelete(profile.id)
              }}
            >
              <Trash2 size={13} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("profiles.delete")}</TooltipContent>
        </Tooltip>
        <ChevronRight size={16} className="text-muted-foreground ml-0.5" />
      </div>
    </div>
  )
}
