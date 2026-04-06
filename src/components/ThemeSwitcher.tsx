import React, { useState } from "react"
import { useTranslation } from "react-i18next"
import { Check, Palette } from "lucide-react"
import { setTheme, getTheme, type Theme } from "@/lib/utils"
import { useConfig } from "@/contexts/ConfigContext"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { cn } from "@/lib/utils"

interface ThemeSwitcherProps {
  onClose: () => void
}

const THEME_COLORS: Record<Theme, string> = {
  default: "hsl(220 13% 12%)",
  light: "hsl(0 0% 98%)",
  ocean: "hsl(200 30% 10%)",
  forest: "hsl(140 25% 12%)",
  sunset: "hsl(20 30% 12%)",
}

export const ThemeSwitcher: React.FC<ThemeSwitcherProps> = ({ onClose }) => {
  const { t } = useTranslation()
  const { updateTheme } = useConfig()
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme())

  const themes: { value: Theme; label: string; description: string }[] = [
    { value: "default", label: t("theme.default"), description: t("theme.defaultDesc") },
    { value: "light", label: t("theme.light"), description: t("theme.lightDesc") },
    { value: "ocean", label: t("theme.ocean"), description: t("theme.oceanDesc") },
    { value: "forest", label: t("theme.forest"), description: t("theme.forestDesc") },
    { value: "sunset", label: t("theme.sunset"), description: t("theme.sunsetDesc") },
  ]

  const handleThemeChange = async (theme: Theme) => {
    setTheme(theme)
    setCurrentTheme(theme)
    try {
      await updateTheme(theme)
    } catch (error) {
      console.error("Failed to save theme:", error)
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Palette size={16} />
            {t("theme.title")}
          </DialogTitle>
        </DialogHeader>

        <div className="grid grid-cols-1 gap-2 py-2">
          {themes.map((theme) => (
            <Card key={theme.value} className="overflow-hidden border-transparent shadow-none">
              <CardContent className="p-0">
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => handleThemeChange(theme.value)}
                  className={cn(
                    "h-auto w-full justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
                    currentTheme === theme.value ? "border-primary bg-accent" : "border-transparent"
                  )}
                >
                  <span
                    className="border-border size-5 shrink-0 rounded-full border"
                    style={{ background: THEME_COLORS[theme.value] }}
                  />
                  <div className="flex flex-col items-start">
                    <span className="text-sm leading-none font-medium">{theme.label}</span>
                    <span className="text-muted-foreground mt-1 text-xs">{theme.description}</span>
                  </div>
                  {currentTheme === theme.value && (
                    <Check size={16} className="text-primary ml-auto" />
                  )}
                </Button>
              </CardContent>
            </Card>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  )
}
