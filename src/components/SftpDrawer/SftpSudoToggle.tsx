import React from "react"
import { Shield, ShieldAlert } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

interface SftpSudoToggleProps {
  enabled: boolean
  disabled: boolean
  onToggle: () => void
}

/**
 * Switches the drawer between working as the user and as root through sudo.
 * Switched on it reads "root" in the destructive color, so a drawer that
 * changes files as root never looks like one that does not.
 */
export const SftpSudoToggle: React.FC<SftpSudoToggleProps> = ({ enabled, disabled, onToggle }) => {
  const { t } = useTranslation()
  const label = enabled ? t("sftp.sudo.disable") : t("sftp.sudo.enable")

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="xs"
          onClick={onToggle}
          disabled={disabled}
          aria-pressed={enabled}
          aria-label={label}
          className={
            enabled
              ? "bg-destructive/15 text-destructive hover:bg-destructive/25 hover:text-destructive"
              : "text-muted-foreground"
          }
        >
          {enabled ? <ShieldAlert /> : <Shield />}
          {enabled ? t("sftp.sudo.root") : t("sftp.sudo.label")}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{enabled ? t("sftp.sudo.active") : label}</TooltipContent>
    </Tooltip>
  )
}
