import React, { useCallback, useEffect, useRef, useState } from "react"
import { Check, ChevronRight, FolderInput, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { SftpDrawerHeaderProps } from "@/components/SftpDrawer/SftpDrawerHeader"
import { Button } from "@/components/ui/button"
import { useKeymap } from "@/contexts/KeymapContext"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { isImeKeyEvent } from "@/lib/ime"
import { cn } from "@/lib/utils"

type SftpPathBarProps = Pick<
  SftpDrawerHeaderProps,
  | "breadcrumbs"
  | "isGlobalShortcutTarget"
  | "isLoading"
  | "listingCurrentPath"
  | "loadDirectory"
  | "visible"
>

/**
 * Where the drawer is: the breadcrumbs, or a field to type a path into
 * (opened by double-clicking the breadcrumbs or by its shortcut).
 */
export function SftpPathBar({
  breadcrumbs,
  isGlobalShortcutTarget,
  isLoading,
  listingCurrentPath,
  loadDirectory,
  visible,
}: SftpPathBarProps) {
  const { t } = useTranslation()
  const { registerHandler } = useKeymap()
  const [isPathEditing, setIsPathEditing] = useState(false)
  const [pathDraft, setPathDraft] = useState(listingCurrentPath ?? "")
  const [pathError, setPathError] = useState<string | null>(null)
  const pathInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (isPathEditing) {
      pathInputRef.current?.focus()
      pathInputRef.current?.select()
    }
  }, [isPathEditing])

  const openPathEditor = useCallback(() => {
    if (!visible || !listingCurrentPath || isLoading) return

    setPathDraft(listingCurrentPath)
    setPathError(null)
    setIsPathEditing(true)
  }, [isLoading, listingCurrentPath, visible])

  const handlePathDisplayDoubleClick = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if ((event.target as HTMLElement).closest("button")) {
        return
      }

      openPathEditor()
    },
    [openPathEditor]
  )

  const closePathEditor = () => {
    setIsPathEditing(false)
    setPathDraft(listingCurrentPath ?? "")
    setPathError(null)
  }

  const submitPath = async () => {
    const nextPath = pathDraft.trim()
    if (!nextPath || isLoading) return

    setPathError(null)
    try {
      await loadDirectory(nextPath, { throwOnError: true })
      setIsPathEditing(false)
    } catch (error) {
      setPathError(error instanceof Error ? error.message : String(error))
    }
  }

  const handlePathKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (isImeKeyEvent(event.nativeEvent)) return
    if (event.key === "Enter") {
      event.preventDefault()
      void submitPath()
      return
    }

    if (event.key === "Escape") {
      event.preventDefault()
      closePathEditor()
    }
  }

  useEffect(() => {
    return registerHandler("sftp.focusPath", () => {
      if (!visible || !isGlobalShortcutTarget || !listingCurrentPath || isLoading) return false
      openPathEditor()
    })
  }, [
    isGlobalShortcutTarget,
    isLoading,
    listingCurrentPath,
    openPathEditor,
    registerHandler,
    visible,
  ])

  return isPathEditing ? (
    <div className="sftp-path-editor">
      <FolderInput className="sftp-path-icon" />
      <Input
        ref={pathInputRef}
        value={pathDraft}
        onChange={(event) => setPathDraft(event.target.value)}
        onKeyDown={handlePathKeyDown}
        disabled={isLoading}
        className={cn("sftp-path-input", pathError && "border-destructive")}
        aria-describedby={pathError ? "sftp-path-error" : undefined}
        aria-invalid={Boolean(pathError)}
        aria-label={t("sftp.path.label", { defaultValue: "Go to remote path" })}
      />
      <div className="sftp-path-controls">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={() => void submitPath()}
              disabled={isLoading || !pathDraft.trim()}
              aria-label={t("sftp.path.go", { defaultValue: "Go to path" })}
            >
              <Check className="size-3" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("sftp.path.go", { defaultValue: "Go to path" })}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={closePathEditor}
              aria-label={t("sftp.path.cancel", { defaultValue: "Cancel path edit" })}
            >
              <X className="size-3" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            {t("sftp.path.cancel", { defaultValue: "Cancel path edit" })}
          </TooltipContent>
        </Tooltip>
      </div>
      {pathError && (
        <span id="sftp-path-error" className="sftp-path-error" role="alert">
          {pathError}
        </span>
      )}
    </div>
  ) : (
    <div className="sftp-path-display" onDoubleClick={handlePathDisplayDoubleClick}>
      <div className="sftp-breadcrumbs">
        {breadcrumbs.map((item, index) => (
          <React.Fragment key={item.path}>
            {index > 0 && <ChevronRight className="text-muted-foreground size-3" />}
            <Button
              variant="ghost"
              size="xs"
              onClick={() => void loadDirectory(item.path)}
              disabled={isLoading}
              className="h-6 px-2"
            >
              {item.label}
            </Button>
          </React.Fragment>
        ))}
      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={openPathEditor}
            disabled={!listingCurrentPath || isLoading}
            aria-label={t("sftp.path.edit", { defaultValue: "Enter remote path" })}
          >
            <FolderInput className="size-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t("sftp.path.edit", { defaultValue: "Enter remote path" })}
        </TooltipContent>
      </Tooltip>
    </div>
  )
}
