import React from "react"
import { FolderSync, SquareTerminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

interface SftpTerminalActionsProps {
  follows: boolean
  toggleFollow: () => void
  /** The directory the terminal's shell last reported, if it reports any. */
  terminalCwd: string | null
  /** The connection installs tTerm's shell integration, which reports it. */
  shellIntegration: boolean
  /** The listed directory, null while there is none or no shell to send it to. */
  currentPath: string | null
  cdInTerminal: (path: string) => void
}

/**
 * The drawer's links to its terminal: following the shell's directory, and
 * taking the shell to the listed one.
 */
export const SftpTerminalActions: React.FC<SftpTerminalActionsProps> = ({
  follows,
  toggleFollow,
  terminalCwd,
  shellIntegration,
  currentPath,
  cdInTerminal,
}) => {
  const { t } = useTranslation()
  const label = t("sftp.follow.label")
  const cdLabel = t("sftp.actions.openCurrentInTerminal")
  // Following waits for a report that may never come; say why.
  const waiting = follows && terminalCwd === null
  const hint = !waiting
    ? null
    : shellIntegration
      ? t("sftp.follow.notReported")
      : t("sftp.follow.needsShellIntegration")

  return (
    <>
      <div className="sftp-action-group">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={follows ? "secondary" : "ghost"}
              size="icon-sm"
              onClick={toggleFollow}
              aria-label={label}
              aria-pressed={follows}
              className="relative"
            >
              <FolderSync className="size-4" />
              {waiting && (
                <span
                  aria-hidden="true"
                  className="absolute top-1 right-1 size-1.5 rounded-full bg-amber-500"
                />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent className="max-w-72">
            <div>{label}</div>
            {hint && <div className="opacity-80">{hint}</div>}
          </TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => currentPath && cdInTerminal(currentPath)}
              disabled={!currentPath}
              aria-label={cdLabel}
            >
              <SquareTerminal className="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{cdLabel}</TooltipContent>
        </Tooltip>
      </div>
      <div className="sftp-action-divider" aria-hidden="true" />
    </>
  )
}
