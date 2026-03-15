import React, { useState } from "react"
import { useTranslation } from "react-i18next"
import { Palette, X } from "lucide-react"
import { setTheme, getTheme, type Theme } from "../lib/utils"

interface ThemeSwitcherProps {
  onClose: () => void
}

export const ThemeSwitcher: React.FC<ThemeSwitcherProps> = ({ onClose }) => {
  const { t } = useTranslation()
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme())

  const themes: { value: Theme; label: string; description: string }[] = [
    { value: "default", label: t("theme.default"), description: t("theme.defaultDesc") },
    { value: "light", label: t("theme.light"), description: t("theme.lightDesc") },
    { value: "ocean", label: t("theme.ocean"), description: t("theme.oceanDesc") },
    { value: "forest", label: t("theme.forest"), description: t("theme.forestDesc") },
    { value: "sunset", label: t("theme.sunset"), description: t("theme.sunsetDesc") },
  ]

  const handleThemeChange = (theme: Theme) => {
    setTheme(theme)
    setCurrentTheme(theme)
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>
            <Palette size={16} style={{ display: "inline", marginRight: "8px" }} />
            {t("theme.title")}
          </h2>
          <button className="dialog-close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>
        <div className="dialog-content">
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            {themes.map((theme) => (
              <div
                key={theme.value}
                className={`connection-type ${currentTheme === theme.value ? "active" : ""}`}
                onClick={() => handleThemeChange(theme.value)}
                style={{
                  flexDirection: "row",
                  justifyContent: "space-between",
                  padding: "16px",
                  cursor: "pointer",
                }}
              >
                <div style={{ textAlign: "left" }}>
                  <div style={{ fontSize: "14px", fontWeight: 600, marginBottom: "4px" }}>
                    {theme.label}
                  </div>
                  <div style={{ fontSize: "12px", opacity: 0.7 }}>{theme.description}</div>
                </div>
                {currentTheme === theme.value && <div style={{ fontSize: "18px" }}>✓</div>}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
