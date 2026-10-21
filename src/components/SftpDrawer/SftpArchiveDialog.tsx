import React from "react"
import { useTranslation } from "react-i18next"

import {
  COMPRESS_EXTENSIONS,
  COMPRESS_FORMATS,
  withArchiveExtension,
  type CompressFormat,
} from "@/components/SftpDrawer/sftpArchive"
import type { SftpArchiveDialogState } from "@/components/SftpDrawer/useSftpArchive"
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
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { isImeKeyEvent } from "@/lib/ime"

interface SftpArchiveDialogProps {
  dialog: SftpArchiveDialogState
  setDialog: React.Dispatch<React.SetStateAction<SftpArchiveDialogState>>
  onConfirm: () => void
}

/** Asks how to compress entries, or where to extract an archive. */
export const SftpArchiveDialog: React.FC<SftpArchiveDialogProps> = ({
  dialog,
  setDialog,
  onConfirm,
}) => {
  const { t } = useTranslation()
  const close = () => setDialog({ type: "none" })
  const confirmOnEnter = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
      event.preventDefault()
      onConfirm()
    }
  }

  return (
    <Dialog open={dialog.type !== "none"} onOpenChange={(open) => !open && close()}>
      <DialogContent>
        {dialog.type === "compress" && (
          <>
            <DialogHeader>
              <DialogTitle>{t("sftp.archive.compressTitle")}</DialogTitle>
              <DialogDescription>
                {t("sftp.archive.compressDescription", {
                  count: dialog.entries.length,
                  folder: dialog.directory,
                })}
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-2">
              <div className="grid gap-2">
                <Label htmlFor="sftp-archive-name">{t("sftp.archive.nameLabel")}</Label>
                <Input
                  id="sftp-archive-name"
                  value={dialog.name}
                  onChange={(event) => setDialog({ ...dialog, name: event.target.value })}
                  onKeyDown={confirmOnEnter}
                  autoFocus
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="sftp-archive-format">{t("sftp.archive.formatLabel")}</Label>
                <Select
                  id="sftp-archive-format"
                  value={dialog.format}
                  onChange={(event) => {
                    const format = event.target.value as CompressFormat
                    setDialog({
                      ...dialog,
                      format,
                      name: withArchiveExtension(dialog.name, dialog.format, format),
                    })
                  }}
                >
                  {COMPRESS_FORMATS.map((format) => (
                    <option key={format} value={format}>
                      {COMPRESS_EXTENSIONS[format]}
                    </option>
                  ))}
                </Select>
              </div>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={close}>
                {t("common.cancel")}
              </Button>
              <Button onClick={onConfirm} disabled={!dialog.name.trim()}>
                {t("sftp.archive.compress")}
              </Button>
            </DialogFooter>
          </>
        )}
        {dialog.type === "extract" && (
          <>
            <DialogHeader>
              <DialogTitle>
                {t("sftp.archive.extractTitle", { name: dialog.entry.name })}
              </DialogTitle>
              <DialogDescription>{t("sftp.archive.extractDescription")}</DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-2">
              <div className="grid gap-2">
                <Label htmlFor="sftp-extract-destination">
                  {t("sftp.archive.destinationLabel")}
                </Label>
                <Input
                  id="sftp-extract-destination"
                  value={dialog.destination}
                  onChange={(event) => setDialog({ ...dialog, destination: event.target.value })}
                  onKeyDown={confirmOnEnter}
                  className="font-mono"
                  autoFocus
                />
              </div>
              <Label className="flex items-center gap-2 font-normal">
                <Checkbox
                  checked={dialog.overwrite}
                  onCheckedChange={(checked) => setDialog({ ...dialog, overwrite: checked })}
                />
                {t("sftp.archive.overwrite")}
              </Label>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={close}>
                {t("common.cancel")}
              </Button>
              <Button onClick={onConfirm} disabled={!dialog.destination.trim()}>
                {t("sftp.archive.extract")}
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
