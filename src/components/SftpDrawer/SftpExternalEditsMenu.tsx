import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { ExternalLink, FolderOpen, FilePen, Upload, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { needsAttention, type ExternalEdit } from "@/components/SftpDrawer/externalEdit"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

interface SftpExternalEditsMenuProps {
  edits: ExternalEdit[]
  onUpload: (edit: ExternalEdit) => void
  onReopen: (edit: ExternalEdit) => void
  onReveal: (edit: ExternalEdit) => void
  onStop: (edit: ExternalEdit) => void
}

const MENU_WIDTH = 416
const VIEWPORT_MARGIN = 8

/**
 * Where the menu opens: under its button, right-aligned to it, and moved
 * along so it stays inside the window.
 */
function placeMenu(trigger: DOMRect): React.CSSProperties {
  const width = Math.min(MENU_WIDTH, window.innerWidth - VIEWPORT_MARGIN * 2)
  const left = Math.min(
    Math.max(VIEWPORT_MARGIN, trigger.right - width),
    window.innerWidth - width - VIEWPORT_MARGIN
  )
  return { top: trigger.bottom + 4, left, width }
}

/** The files open in other programs, shown while there are any. */
export const SftpExternalEditsMenu: React.FC<SftpExternalEditsMenuProps> = ({
  edits,
  onUpload,
  onReopen,
  onReveal,
  onStop,
}) => {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const [menuStyle, setMenuStyle] = useState<React.CSSProperties | null>(null)

  const updatePosition = useCallback(() => {
    const root = rootRef.current
    if (root) setMenuStyle(placeMenu(root.getBoundingClientRect()))
  }, [])

  // The menu goes to the body: the drawer's pane clips what overflows it, and
  // the button can sit near the window's left edge in a narrow pane.
  useLayoutEffect(() => {
    if (!open) return
    updatePosition()
    window.addEventListener("resize", updatePosition)
    window.addEventListener("scroll", updatePosition, true)
    return () => {
      window.removeEventListener("resize", updatePosition)
      window.removeEventListener("scroll", updatePosition, true)
    }
  }, [open, updatePosition])

  useEffect(() => {
    if (!open) return
    const closeOutside = (event: MouseEvent) => {
      const target = event.target as Node
      if (!rootRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false)
    }
    document.addEventListener("mousedown", closeOutside)
    document.addEventListener("keydown", closeOnEscape)
    return () => {
      document.removeEventListener("mousedown", closeOutside)
      document.removeEventListener("keydown", closeOnEscape)
    }
  }, [open])

  if (edits.length === 0) return null
  const label = t("sftp.externalEdit.menu", { count: edits.length })

  return (
    <div ref={rootRef} className="sftp-action-group relative">
      <Button
        type="button"
        variant={open ? "secondary" : "ghost"}
        size="icon-sm"
        aria-label={label}
        title={label}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
        className="relative"
      >
        <FilePen className="size-4" />
        <span
          aria-hidden="true"
          className={cn(
            "absolute top-0.5 right-0.5 min-w-3.5 rounded-full px-0.5 text-[9px] leading-3.5 font-semibold",
            needsAttention(edits)
              ? "bg-amber-500 text-white"
              : "bg-muted-foreground/30 text-foreground"
          )}
        >
          {edits.length}
        </span>
      </Button>
      {open &&
        menuStyle &&
        createPortal(
          <div
            ref={menuRef}
            role="menu"
            style={menuStyle}
            className="bg-popover text-popover-foreground border-border fixed z-50 rounded-md border p-1 shadow-lg"
          >
            <div className="text-muted-foreground px-2 py-1 text-[11px]">{label}</div>
            <div className="max-h-72 overflow-y-auto">
              {edits.map((edit) => (
                <EditRow
                  key={edit.id}
                  edit={edit}
                  onUpload={onUpload}
                  onReopen={onReopen}
                  onReveal={onReveal}
                  onStop={onStop}
                />
              ))}
            </div>
          </div>,
          document.body
        )}
    </div>
  )
}

const EditRow: React.FC<{ edit: ExternalEdit } & Omit<SftpExternalEditsMenuProps, "edits">> = ({
  edit,
  onUpload,
  onReopen,
  onReveal,
  onStop,
}) => {
  const { t } = useTranslation()
  const status =
    edit.status === "failed" && edit.error
      ? t("sftp.externalEdit.status.failedWith", { error: edit.error })
      : t(`sftp.externalEdit.status.${edit.status}`)
  const actions = [
    {
      label: t("sftp.externalEdit.upload"),
      icon: <Upload />,
      run: onUpload,
      disabled: edit.status === "uploading",
    },
    { label: t("sftp.externalEdit.reopen"), icon: <ExternalLink />, run: onReopen },
    { label: t("sftp.externalEdit.reveal"), icon: <FolderOpen />, run: onReveal },
    { label: t("sftp.externalEdit.stop"), icon: <X />, run: onStop },
  ]

  return (
    <div role="menuitem" className="hover:bg-muted flex items-center gap-2 rounded px-2 py-1">
      <div className="min-w-0 flex-1">
        <div className="truncate text-xs font-medium" title={edit.remotePath}>
          {edit.fileName}
        </div>
        <div
          className={cn(
            "truncate text-[11px]",
            edit.status === "failed"
              ? "text-destructive"
              : edit.status === "modified"
                ? "text-amber-600 dark:text-amber-400"
                : "text-muted-foreground"
          )}
          title={status}
        >
          {status}
        </div>
      </div>
      {actions.map((action) => (
        <Button
          key={action.label}
          type="button"
          size="icon-xs"
          variant="ghost"
          aria-label={action.label}
          title={action.label}
          disabled={action.disabled}
          onClick={() => action.run(edit)}
        >
          {action.icon}
        </Button>
      ))}
    </div>
  )
}
