import React from "react"
import { useTranslation } from "react-i18next"
import { Languages, X } from "lucide-react"
import { useConfig } from "@/contexts/ConfigContext"

const languages = [
  { code: "en", label: "English", nativeLabel: "English" },
  { code: "zh", label: "Chinese", nativeLabel: "Chinese" },
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
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>
            <Languages size={16} style={{ display: "inline", marginRight: "8px" }} />
            {t("language.title")}
          </h2>
          <button className="dialog-close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>
        <div className="dialog-content">
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            {languages.map((lang) => (
              <div
                key={lang.code}
                className={`connection-type ${i18n.language === lang.code ? "active" : ""}`}
                onClick={() => handleLanguageChange(lang.code)}
                style={{
                  flexDirection: "row",
                  justifyContent: "space-between",
                  padding: "16px",
                  cursor: "pointer",
                }}
              >
                <div style={{ textAlign: "left" }}>
                  <div style={{ fontSize: "14px", fontWeight: 600, marginBottom: "4px" }}>
                    {lang.nativeLabel}
                  </div>
                  <div style={{ fontSize: "12px", opacity: 0.7 }}>{lang.label}</div>
                </div>
                {i18n.language === lang.code && <div style={{ fontSize: "18px" }}>✓</div>}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
