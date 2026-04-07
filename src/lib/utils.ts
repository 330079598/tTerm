import {type ClassValue, clsx} from "clsx"
import {twMerge} from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Theme management
export type Theme = "default" | "light" | "ocean" | "forest" | "sunset" | "ubuntu"

export function setTheme(theme: Theme) {
  const root = document.documentElement
  root.setAttribute("data-theme", theme)
}

export function getTheme(): Theme {
  return (document.documentElement.getAttribute("data-theme") as Theme) || "default"
}

// HSL color helper
export function hslToCssColor(hsl: string): string {
  return `hsl(${hsl})`
}
