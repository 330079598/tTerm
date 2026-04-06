import React from "react"
import { useTranslation } from "react-i18next"
import { Languages, Check } from "lucide-react"
import { useConfig } from "@/contexts/ConfigContext"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { cn } from "@/lib/utils"

const languages = [
  { code: "en", label: "English", nativeLabel: "English" },
  { code: "zh", label: "中文", nativeLabel: "Chinese" },
]

interface LanguageSwitcherProps {
  onClose: () => void
}

export const LanguageSwitcher: React.FC<LanguageSwitcherProps> = ({ onClose }) => {
  const { t, i18n } = useTranslation()
  const { updateLanguage } = useConfig()

  const handleLanguageChange = async (langCode: string) => {
    await i18n.changeLanguage(langCode)
    try {
      await updateLanguage(langCode)
    } catch (error) {
      console.error("Failed to save language:", error)
    }
    onClose()
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-xs">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Languages size={16} />
            {t("language.title")}
          </DialogTitle>
        </DialogHeader>

        <div className="flex flex-col gap-2 py-2">
          {languages.map((lang) => {
            const isActive = i18n.language === lang.code
            return (
              <Card key={lang.code} className="overflow-hidden border-transparent shadow-none">
                <CardContent className="p-0">
                  <Button
                    key={lang.code}
                    type="button"
                    variant="ghost"
                    onClick={() => handleLanguageChange(lang.code)}
                    className={cn(
                      "h-auto w-full justify-between rounded-lg px-4 py-3 text-left",
                      isActive && "bg-muted"
                    )}
                  >
                    <div>
                      <div className="text-sm font-semibold">{lang.nativeLabel}</div>
                      <div className="text-muted-foreground text-xs">{lang.label}</div>
                    </div>
                    {isActive && <Check size={16} className="text-primary" />}
                  </Button>
                </CardContent>
              </Card>
            )
          })}
        </div>
      </DialogContent>
    </Dialog>
  )
}
