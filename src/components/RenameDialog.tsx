import React, { useState, useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"
import { X } from "lucide-react"

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
  const inputRef = useRef<HTMLInputElement>(null)

  // Reset name when dialog opens or currentName changes
  useEffect(() => {
    setName(currentName)
  }, [currentName])

  // Focus input when dialog opens
  useEffect(() => {
    if (isOpen) {
      setTimeout(() => {
        inputRef.current?.focus()
        inputRef.current?.select()
      }, 100)
    }
  }, [isOpen])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const trimmedName = name.trim()
    if (trimmedName && trimmedName !== currentName) {
      onConfirm(trimmedName)
    }
    onClose()
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose()
    }
  }

  if (!isOpen) return null

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()} style={{ maxWidth: "400px" }}>
        <div className="dialog-header">
          <h2>{t("rename.title")}</h2>
          <button className="dialog-close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="dialog-content">
            <div className="form-group">
              <label htmlFor="tab-name">{t("rename.label")}</label>
              <input
                ref={inputRef}
                id="tab-name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder={t("rename.label")}
                autoComplete="off"
              />
            </div>
          </div>
          <div className="dialog-actions">
            <button type="button" className="btn-secondary" onClick={onClose}>
              {t("rename.cancel")}
            </button>
            <button type="submit" className="btn-primary" disabled={!name.trim()}>
              {t("rename.confirm")}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
