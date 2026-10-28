import React from "react"
import { useTranslation } from "react-i18next"

import type { FontSettingsTabProps } from "@/components/SettingsDialog/FontSettingsTab"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { firstInstalledFont } from "@/lib/terminalFont"
import { cn } from "@/lib/utils"

const NERD_FONT_PATTERN =
  /nerd\s*font|nerdfont|nf\b|nerd|powerline|meslo\s+lg.*nerd|fira\s*code.*nerd|jetbrains.*nerd|hack.*nerd|iosevka.*nerd|cascadia.*nerd|roboto.*mono.*nerd|ubuntu.*mono.*nerd|source\s*code\s*pro.*nerd|dejavu.*sans.*mono.*nerd|liberation.*mono.*nerd|noto.*sans.*mono.*nerd/i

function isNerdFont(name: string): boolean {
  return NERD_FONT_PATTERN.test(name)
}

type FontFamilyPickerProps = Pick<
  FontSettingsTabProps,
  "fontFamily" | "setFontFamily" | "systemFonts" | "loadingFonts" | "fontLoadError"
>

/** The terminal font: typed in, or picked from the system's fonts (Nerd Fonts marked). */
export function FontFamilyPicker({
  fontFamily,
  setFontFamily,
  systemFonts,
  loadingFonts,
  fontLoadError,
}: FontFamilyPickerProps) {
  const { t } = useTranslation()
  const [fontSearchQuery, setFontSearchQuery] = React.useState("")
  const [showNerdFontsOnly, setShowNerdFontsOnly] = React.useState(false)

  const nerdFontCount = React.useMemo(
    () => systemFonts.filter((f) => isNerdFont(f)).length,
    [systemFonts]
  )

  const filteredFonts = React.useMemo(() => {
    let fonts = systemFonts
    if (fontSearchQuery.trim()) {
      const query = fontSearchQuery.toLowerCase()
      fonts = fonts.filter((font) => font.toLowerCase().includes(query))
    }
    if (showNerdFontsOnly) {
      fonts = fonts.filter((font) => isNerdFont(font))
    }
    return fonts
  }, [systemFonts, fontSearchQuery, showNerdFontsOnly])

  return (
    <div>
      <Label className="mb-2 block">{t("fontSettings.fontFamily")}</Label>
      <Input
        type="text"
        value={fontFamily}
        onChange={(e) => setFontFamily(e.target.value)}
        placeholder={t("fontSettings.customFont")}
        className="mb-2"
      />
      {nerdFontCount > 0 && !isNerdFont(firstInstalledFont(fontFamily, systemFonts)) && (
        <p className="mb-2 text-xs text-amber-600 dark:text-amber-400">
          {t("fontSettings.nerdFontWarning", {
            defaultValue:
              "Current font may not support Nerd Font characters (used by powerlevel10k, starship, etc.). Select a font with the NF badge below.",
          })}
        </p>
      )}

      {loadingFonts ? (
        <p className="text-muted-foreground text-xs">{t("fontSettings.loadingFonts")}</p>
      ) : (
        <>
          {fontLoadError ? (
            <p className="mb-2 text-xs text-amber-600 dark:text-amber-400">{fontLoadError}</p>
          ) : null}
          <Input
            type="text"
            value={fontSearchQuery}
            onChange={(e) => setFontSearchQuery(e.target.value)}
            placeholder={t("fontSettings.searchFonts")}
            className="mb-2"
          />
          {nerdFontCount > 0 && (
            <button
              type="button"
              onClick={() => setShowNerdFontsOnly(!showNerdFontsOnly)}
              className={cn(
                "mb-2 inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors",
                showNerdFontsOnly
                  ? "bg-primary text-primary-foreground"
                  : "bg-secondary text-secondary-foreground hover:bg-secondary/80"
              )}
            >
              Nerd Font {showNerdFontsOnly ? `(${filteredFonts.length})` : `(${nerdFontCount})`}
            </button>
          )}
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
                      "flex w-full items-center justify-between rounded px-3 py-1.5 text-left text-sm transition-colors",
                      fontFamily.includes(font)
                        ? "bg-primary/15 text-foreground"
                        : "text-muted-foreground hover:bg-muted hover:text-foreground"
                    )}
                    style={{ fontFamily: font }}
                  >
                    <span>{font}</span>
                    {isNerdFont(font) && (
                      <span className="ml-2 shrink-0 rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-medium text-emerald-600 dark:text-emerald-400">
                        NF
                      </span>
                    )}
                  </button>
                ))
              )}
            </div>
          </ScrollArea>
        </>
      )}
    </div>
  )
}
