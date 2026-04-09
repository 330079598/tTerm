import React from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"

interface FontSettingsTabProps {
  fontFamily: string
  fontSize: number
  handleFontSave: () => Promise<void>
  loadingFonts: boolean
  scrollbackLines: number
  setFontFamily: React.Dispatch<React.SetStateAction<string>>
  setFontSize: React.Dispatch<React.SetStateAction<number>>
  setScrollbackLines: React.Dispatch<React.SetStateAction<number>>
  systemFonts: string[]
  fontSizeOptions: number[]
}

export const FontSettingsTab: React.FC<FontSettingsTabProps> = ({
  fontFamily,
  fontSize,
  handleFontSave,
  loadingFonts,
  scrollbackLines,
  setFontFamily,
  setFontSize,
  setScrollbackLines,
  systemFonts,
  fontSizeOptions,
}) => {
  const { t } = useTranslation()
  const [fontSearchQuery, setFontSearchQuery] = React.useState("")

  const filteredFonts = React.useMemo(() => {
    if (!fontSearchQuery.trim()) return systemFonts
    const query = fontSearchQuery.toLowerCase()
    return systemFonts.filter((font) => font.toLowerCase().includes(query))
  }, [systemFonts, fontSearchQuery])

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-5">
        <div>
          <Label className="mb-2 block">{t("fontSettings.fontSize")}</Label>
          <div className="flex flex-wrap gap-1.5">
            {fontSizeOptions.map((size) => (
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
                const value = parseInt(e.target.value)
                if (!isNaN(value) && value >= 6 && value <= 72) setFontSize(value)
              }}
              className="h-7 w-16 px-2 text-xs"
            />
          </div>
        </div>

        <div>
          <Label className="mb-2 block">
            {t("fontSettings.scrollbackLines", { defaultValue: "Scrollback Lines" })}
          </Label>
          <div className="space-y-2">
            <Input
              type="number"
              min={100}
              max={1000000}
              value={scrollbackLines}
              onChange={(e) => {
                const value = parseInt(e.target.value)
                if (!isNaN(value) && value >= 100 && value <= 1000000) setScrollbackLines(value)
              }}
              className="w-full"
            />
            <p className="text-muted-foreground text-xs">
              {t("fontSettings.scrollbackLinesDesc", {
                defaultValue:
                  "Number of lines to keep in terminal history (100 - 1,000,000). Higher values use more memory.",
              })}
            </p>
            <div className="flex flex-wrap gap-1.5">
              {[1000, 5000, 10000, 50000, 100000].map((lines) => (
                <Button
                  key={lines}
                  type="button"
                  variant={scrollbackLines === lines ? "default" : "outline"}
                  size="xs"
                  onClick={() => setScrollbackLines(lines)}
                  className={cn(
                    "min-w-[3.5rem]",
                    scrollbackLines !== lines && "text-muted-foreground"
                  )}
                >
                  {lines >= 1000 ? `${lines / 1000}k` : lines}
                </Button>
              ))}
            </div>
          </div>
        </div>

        <div>
          <Label className="mb-2 block">{t("fontSettings.fontFamily")}</Label>
          <Input
            type="text"
            value={fontFamily}
            onChange={(e) => setFontFamily(e.target.value)}
            placeholder={t("fontSettings.customFont")}
            className="mb-2"
          />

          {loadingFonts ? (
            <p className="text-muted-foreground text-xs">{t("fontSettings.loadingFonts")}</p>
          ) : (
            <>
              <Input
                type="text"
                value={fontSearchQuery}
                onChange={(e) => setFontSearchQuery(e.target.value)}
                placeholder={t("fontSettings.searchFonts")}
                className="mb-2"
              />
              <ScrollArea className="border-border h-48 rounded border">
                <div className="p-1">
                  {systemFonts.length === 0 ? (
                    <p className="text-muted-foreground px-2 py-4 text-center text-xs">
                      {t("fontSettings.noFontsFound")}
                    </p>
                  ) : filteredFonts.length === 0 ? (
                    <p className="text-muted-foreground px-2 py-4 text-center text-xs">
                      {t("fontSettings.noMatchingFonts")}
                    </p>
                  ) : (
                    filteredFonts.map((font) => (
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
            </>
          )}
        </div>

        <div>
          <Label className="mb-2 block">{t("fontSettings.preview")}</Label>
          <div
            className="bg-secondary text-foreground border-border rounded-lg border px-4 py-3"
            style={{ fontFamily, fontSize: `${fontSize}px` }}
          >
            The quick brown fox jumps over the lazy dog 0123456789
          </div>
        </div>

        <Button onClick={handleFontSave} className="w-full">
          {t("fontSettings.save")}
        </Button>
      </div>
    </ScrollArea>
  )
}
