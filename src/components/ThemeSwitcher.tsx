import React, { useState } from "react"
import { Palette, X } from "lucide-react"
import { setTheme, getTheme, type Theme } from "../lib/utils"

const themes: { value: Theme; label: string; description: string }[] = [
  { value: "default", label: "Default", description: "Dark theme inspired by Tabby" },
  { value: "light", label: "Light", description: "Clean light theme" },
  { value: "ocean", label: "Ocean", description: "Deep blue ocean theme" },
  { value: "forest", label: "Forest", description: "Natural green theme" },
  { value: "sunset", label: "Sunset", description: "Warm orange and red theme" },
]

interface ThemeSwitcherProps {
  onClose: () => void
}

export const ThemeSwitcher: React.FC<ThemeSwitcherProps> = ({ onClose }) => {
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme())

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
            Choose Theme
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
