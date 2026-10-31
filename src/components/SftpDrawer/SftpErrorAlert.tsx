import React from "react"
import { AlertCircle, ShieldAlert } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"

interface SftpErrorAlertProps {
  error: string | null
  /**
   * Offers to try again as root: set when the drawer works as the user and the
   * server said the user may not do what was asked.
   */
  onOpenAsRoot?: () => void
}

/** The drawer's error, with a way past it through sudo when the account lacked the permission. */
export const SftpErrorAlert: React.FC<SftpErrorAlertProps> = ({ error, onOpenAsRoot }) => {
  const { t } = useTranslation()
  if (!error) return null

  return (
    <Alert className="bg-destructive/10 text-destructive rounded-none border-x-0 border-t-0">
      <AlertCircle className="size-4" />
      <AlertDescription className="flex flex-wrap items-center justify-between gap-2">
        <span>{error}</span>
        {onOpenAsRoot && (
          <Button type="button" variant="outline" size="xs" onClick={onOpenAsRoot}>
            <ShieldAlert />
            {t("sftp.sudo.openAsRoot")}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  )
}
