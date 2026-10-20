import React from "react"
import { FolderSync } from "lucide-react"
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
}

/** The drawer's link to its terminal: following the shell's directory. */
export const SftpTerminalActions: React.FC<SftpTerminalActionsProps> = ({
  follows,
  toggleFollow,
  terminalCwd,
  shellIntegration,
}) => {
  const { t } = useTranslation()
  const label = t("sftp.follow.label")
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
      </div>
      <div className="sftp-action-divider" aria-hidden="true" />
    </>
  )
}
