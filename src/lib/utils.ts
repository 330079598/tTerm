// Theme management
export type Theme = "default" | "light" | "ocean" | "forest" | "sunset"

export function setTheme(theme: Theme) {
  const root = document.documentElement
  root.setAttribute("data-theme", theme)
}

export function getTheme(): Theme {
  return (document.documentElement.getAttribute("data-theme") as Theme) || "default"
}
