import React, { useState, useEffect } from "react"
import { Type } from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import { useConfig } from "@/contexts/ConfigContext"
import { useTranslation } from "react-i18next"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

interface FontSettingsProps {
  onClose: () => void
}

const FONT_SIZE_OPTIONS = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24]

export const FontSettings: React.FC<FontSettingsProps> = ({ onClose }) => {
  const { config, saveConfig } = useConfig()
  const { t } = useTranslation()
  const [fontFamily, setFontFamily] = useState(config.font_family)
  const [fontSize, setFontSize] = useState(config.font_size)
  const [systemFonts, setSystemFonts] = useState<string[]>([])
  const [loadingFonts, setLoadingFonts] = useState(true)

  useEffect(() => {
    invoke<string[]>("list_fonts")
      .then((fonts) => setSystemFonts(fonts))
      .catch(() => setSystemFonts([]))
      .finally(() => setLoadingFonts(false))
  }, [])

  const handleSave = async () => {
    await saveConfig({ font_family: fontFamily, font_size: fontSize })
    onClose()
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Type size={16} />
            {t("fontSettings.title")}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-5 py-2">
          {/* Font Size */}
          <div>
            <label className="mb-2 block text-sm font-medium">{t("fontSettings.fontSize")}</label>
            <div className="flex flex-wrap gap-1.5">
              {FONT_SIZE_OPTIONS.map((size) => (
                <button
                  key={size}
                  onClick={() => setFontSize(size)}
                  className={cn(
                    "hover:bg-accent h-7 min-w-[2.25rem] rounded border px-2 text-xs transition-colors",
                    fontSize === size
                      ? "border-primary bg-accent text-foreground"
                      : "border-border text-muted-foreground"
                  )}
                >
                  {size}
                </button>
              ))}
            </div>
          </div>

          {/* Font Family */}
          <div>
            <label className="mb-2 block text-sm font-medium">{t("fontSettings.fontFamily")}</label>

            {/* Custom input */}
            <input
              type="text"
              value={fontFamily}
              onChange={(e) => setFontFamily(e.target.value)}
              placeholder={t("fontSettings.customFont")}
              className="border-border bg-background focus:border-primary mb-2 w-full rounded border px-3 py-1.5 text-sm outline-none"
            />

            {/* System fonts list */}
            {loadingFonts ? (
              <p className="text-muted-foreground text-xs">{t("fontSettings.loadingFonts")}</p>
            ) : (
              <ScrollArea className="border-border h-48 rounded border">
                <div className="p-1">
                  {systemFonts.length === 0 ? (
                    <p className="text-muted-foreground px-2 py-4 text-center text-xs">
                      {t("fontSettings.noFontsFound")}
                    </p>
                  ) : (
                    systemFonts.map((font) => (
                      <button
                        key={font}
                        onClick={() => setFontFamily(`"${font}", monospace`)}
                        className={cn(
                          "hover:bg-accent w-full rounded px-3 py-1.5 text-left text-sm transition-colors",
                          fontFamily.includes(font)
                            ? "bg-accent text-foreground"
                            : "text-muted-foreground"
                        )}
                        style={{ fontFamily: font }}
                      >
                        {font}
                      </button>
                    ))
                  )}
                </div>
              </ScrollArea>
            )}
          </div>

          {/* Preview */}
          <div>
            <label className="mb-2 block text-sm font-medium">{t("fontSettings.preview")}</label>
            <div
              className="border-border bg-secondary text-foreground rounded border px-4 py-3"
              style={{ fontFamily, fontSize: `${fontSize}px` }}
            >
              The quick brown fox jumps over the lazy dog 0123456789
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("fontSettings.cancel")}
          </Button>
          <Button onClick={handleSave}>{t("fontSettings.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
