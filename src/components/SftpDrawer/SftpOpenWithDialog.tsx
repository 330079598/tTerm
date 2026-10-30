import React, { useState } from "react"
import { AppWindow } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { AppChoice } from "@/components/SftpDrawer/externalEdit"
import type { OpenWithState } from "@/components/SftpDrawer/useSftpOpenWith"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"

interface SftpOpenWithDialogProps {
  state: OpenWithState | null
  onClose: () => void
  onPick: (app: AppChoice, asEditor: boolean) => void
  onOther: () => void
}

/** The applications that open a file, as Finder's Open With lists them. */
export const SftpOpenWithDialog: React.FC<SftpOpenWithDialogProps> = ({
  state,
  onClose,
  onPick,
  onOther,
}) => {
  const { t } = useTranslation()
  const [asEditor, setAsEditor] = useState(false)

  return (
    <Dialog open={state !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("sftp.openWith.title", { name: state?.entry.name ?? "" })}</DialogTitle>
          <DialogDescription>{t("sftp.openWith.description")}</DialogDescription>
        </DialogHeader>
        <div className="-mx-1 max-h-80 overflow-y-auto">
          {state?.apps.map((app) => (
            <button
              key={app.path}
              type="button"
              title={app.path}
              className="hover:bg-muted focus-visible:bg-muted flex w-full items-center gap-3 rounded-md px-2 py-1.5 text-left outline-none"
              onClick={() => onPick(app, asEditor)}
            >
              {app.icon ? (
                <img src={app.icon} alt="" className="size-8 shrink-0" />
              ) : (
                <AppWindow className="text-muted-foreground size-8 shrink-0 p-1" />
              )}
              <span className="min-w-0 flex-1 truncate text-sm">{app.name}</span>
              {app.isDefault && (
                <span className="text-muted-foreground shrink-0 text-xs">
                  {t("sftp.openWith.default")}
                </span>
              )}
            </button>
          ))}
        </div>
        <Label className="flex items-center gap-2 text-sm font-normal">
          <Checkbox checked={asEditor} onCheckedChange={setAsEditor} />
          {t("sftp.openWith.useAsEditor")}
        </Label>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onOther}>
            {t("sftp.openWith.other")}
          </Button>
          <Button type="button" variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
