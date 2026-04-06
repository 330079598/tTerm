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
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
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
      <DialogContent className="sm:max-w-lg" onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Type size={16} />
            {t("fontSettings.title")}
          </DialogTitle>
        </DialogHeader>

        <div className="max-h-[70vh] space-y-5 overflow-y-auto py-2">
          {/* Font Size */}
          <div>
            <Label className="mb-2 block">{t("fontSettings.fontSize")}</Label>
            <div className="flex flex-wrap gap-1.5">
              {FONT_SIZE_OPTIONS.map((size) => (
                <Button
                  key={size}
                  type="button"
                  variant={fontSize === size ? "default" : "outline"}
                  size="xs"
                  onClick={() => setFontSize(size)}
                  className={cn("min-w-[2.25rem]", fontSize !== size && "text-muted-foreground")}
                >
                  {size}
                </Button>
              ))}
              <Input
                type="number"
                min={6}
                max={72}
                value={fontSize}
                onChange={(e) => {
                  const v = parseInt(e.target.value)
                  if (!isNaN(v) && v >= 6 && v <= 72) setFontSize(v)
                }}
                className="h-7 w-16 px-2 text-xs"
              />
            </div>
          </div>

          {/* Font Family */}
          <div>
            <Label className="mb-2 block">{t("fontSettings.fontFamily")}</Label>

            {/* Custom input */}
            <Input
              type="text"
              value={fontFamily}
              onChange={(e) => setFontFamily(e.target.value)}
              placeholder={t("fontSettings.customFont")}
              className="mb-2"
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
            <Label className="mb-2 block">{t("fontSettings.preview")}</Label>
            <Card>
              <CardContent
                className="bg-secondary text-foreground px-4 py-3"
                style={{ fontFamily, fontSize: `${fontSize}px` }}
              >
                The quick brown fox jumps over the lazy dog 0123456789
              </CardContent>
            </Card>
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
