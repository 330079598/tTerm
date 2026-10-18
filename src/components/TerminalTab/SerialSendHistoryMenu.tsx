import React, { useEffect, useRef, useState } from "react"
import { History, Trash2, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import type { SerialSend } from "@/lib/serialSendStore"

interface SerialSendHistoryMenuProps {
  history: SerialSend[]
  onPick: (send: SerialSend) => void
  onDelete: (id: string) => void
  onClear: () => void
}

/** The send bar's history: pick a send to put it back, or forget sends. */
export const SerialSendHistoryMenu: React.FC<SerialSendHistoryMenuProps> = ({
  history,
  onPick,
  onDelete,
  onClear,
}) => {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const closeOutside = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false)
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

  const clear = () => {
    if (!window.confirm(t("serialSend.historyClearConfirm"))) return
    setOpen(false)
    onClear()
  }

  return (
    <div ref={rootRef} className="relative">
      <Button
        type="button"
        size="icon-xs"
        variant="ghost"
        aria-label={t("serialSend.history")}
        title={t("serialSend.history")}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        <History />
      </Button>
      {open && (
        <div
          role="menu"
          className="bg-popover text-popover-foreground border-border absolute top-full left-0 z-50 mt-1 w-[min(22rem,calc(100vw-2rem))] rounded-md border p-1 shadow-lg"
        >
          {history.length === 0 ? (
            <div className="text-muted-foreground px-2 py-1.5">{t("serialSend.historyEmpty")}</div>
          ) : (
            <>
              <div className="max-h-64 overflow-y-auto">
                {history.map((send) => (
                  <div
                    key={send.id}
                    className="hover:bg-accent flex items-center gap-1 rounded pl-2"
                  >
                    <button
                      type="button"
                      role="menuitem"
                      className="flex min-w-0 flex-1 items-center gap-2 py-1 text-left"
                      onClick={() => {
                        setOpen(false)
                        onPick(send)
                      }}
                    >
                      <span className="text-muted-foreground w-8 shrink-0 text-[10px]">
                        {send.mode === "hex" ? "HEX" : t("serialSend.text")}
                      </span>
                      <span className="truncate font-mono">{send.text}</span>
                    </button>
                    <Button
                      type="button"
                      size="icon-xs"
                      variant="ghost"
                      aria-label={t("serialSend.historyDelete")}
                      title={t("serialSend.historyDelete")}
                      onClick={() => onDelete(send.id)}
                    >
                      <X />
                    </Button>
                  </div>
                ))}
              </div>
              <div className="border-border mt-1 border-t pt-1">
                <button
                  type="button"
                  role="menuitem"
                  className="hover:bg-accent text-destructive flex w-full items-center gap-2 rounded px-2 py-1 text-left"
                  onClick={clear}
                >
                  <Trash2 className="size-3" />
                  {t("serialSend.historyClear")}
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  )
}
