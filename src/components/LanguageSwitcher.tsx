import React from "react"
import { useTranslation } from "react-i18next"
import { Languages, Check } from "lucide-react"
import { useConfig } from "@/contexts/ConfigContext"
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
              <button
                key={lang.code}
                onClick={() => handleLanguageChange(lang.code)}
                className={cn(
                  "flex items-center justify-between rounded-md px-4 py-3 text-left transition-colors",
                  "hover:bg-muted",
                  isActive && "bg-muted"
                )}
              >
                <div>
                  <div className="text-sm font-semibold">{lang.nativeLabel}</div>
                  <div className="text-muted-foreground text-xs">{lang.label}</div>
                </div>
                {isActive && <Check size={16} className="text-primary" />}
              </button>
            )
          })}
        </div>
      </DialogContent>
    </Dialog>
  )
}
