import React, { useState, useEffect } from "react"
import { useTranslation } from "react-i18next"
import { Edit2 } from "lucide-react"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

interface RenameDialogProps {
  isOpen: boolean
  currentName: string
  onConfirm: (newName: string) => void
  onClose: () => void
}

export const RenameDialog: React.FC<RenameDialogProps> = ({
  isOpen,
  currentName,
  onConfirm,
  onClose,
}) => {
  const { t } = useTranslation()
  const [name, setName] = useState(currentName)

  useEffect(() => {
    setName(currentName)
  }, [currentName])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const trimmedName = name.trim()
    if (trimmedName && trimmedName !== currentName) {
      onConfirm(trimmedName)
    }
    onClose()
  }

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Edit2 size={16} />
            {t("rename.title")}
          </DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <div className="py-2">
            <Label htmlFor="rename-input" className="mb-2 block">
              {t("rename.newName")}
            </Label>
            <Input
              id="rename-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={currentName}
              autoFocus
            />
          </div>

          <DialogFooter className="mt-4">
            <Button type="button" variant="ghost" onClick={onClose}>
              {t("rename.cancel")}
            </Button>
            <Button type="submit" disabled={!name.trim()}>
              {t("rename.confirm")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
