import type {CustomTheme, PresetThemeId, ThemeColors} from "@/types/theme"

// 从 CSS 变量中提取当前主题的颜色值
export function extractThemeColors(): ThemeColors {
  const root = document.documentElement
  const style = getComputedStyle(root)

  const getVar = (name: string): string => {
    return style.getPropertyValue(`--${name}`).trim()
  }

  return {
    background: getVar("background"),
    foreground: getVar("foreground"),
    card: getVar("card"),
    cardForeground: getVar("card-foreground"),
    popover: getVar("popover"),
    popoverForeground: getVar("popover-foreground"),
    primary: getVar("primary"),
    primaryForeground: getVar("primary-foreground"),
    secondary: getVar("secondary"),
    secondaryForeground: getVar("secondary-foreground"),
    muted: getVar("muted"),
    mutedForeground: getVar("muted-foreground"),
    accent: getVar("accent"),
    accentForeground: getVar("accent-foreground"),
    destructive: getVar("destructive"),
    destructiveForeground: getVar("destructive-foreground"),
    success: getVar("success"),
    successForeground: getVar("success-foreground"),
    warning: getVar("warning"),
    warningForeground: getVar("warning-foreground"),
    border: getVar("border"),
    input: getVar("input"),
    ring: getVar("ring"),
    tabBackground: getVar("tab-background"),
    tabActive: getVar("tab-active"),
    tabHover: getVar("tab-hover"),
    titlebar: getVar("titlebar"),
  }
}

// 应用自定义主题颜色到 DOM
export function applyCustomTheme(colors: ThemeColors) {
  const root = document.documentElement

  Object.entries(colors).forEach(([key, value]) => {
    // 将 camelCase 转换为 kebab-case
    const cssVar = key.replace(/([A-Z])/g, "-$1").toLowerCase()
    root.style.setProperty(`--${cssVar}`, value)
  })
}

// 生成唯一的主题 ID
export function generateThemeId(): string {
  return `custom-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`
}

// 从预设主题创建自定义主题
export function createCustomThemeFromPreset(
  presetId: PresetThemeId,
  name: string,
  description?: string
): Omit<CustomTheme, "id"> {
  // 临时切换到预设主题以提取颜色
  const currentTheme = document.documentElement.getAttribute("data-theme")
  document.documentElement.setAttribute("data-theme", presetId)

  const colors = extractThemeColors()

  // 恢复原主题
  if (currentTheme) {
    document.documentElement.setAttribute("data-theme", currentTheme)
  }

  return {
    name,
    description,
    colors,
    baseTheme: presetId,
    isCustom: true,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  }
}

// HSL 颜色转换辅助函数
export function hslToString(h: number, s: number, l: number): string {
  return `${h} ${s}% ${l}%`
}

export function parseHsl(hsl: string): { h: number; s: number; l: number } | null {
  const match = hsl.match(/(\d+\.?\d*)\s+(\d+\.?\d*)%?\s+(\d+\.?\d*)%?/)
  if (!match) return null

  return {
    h: parseFloat(match[1]),
    s: parseFloat(match[2]),
    l: parseFloat(match[3]),
  }
}

// 颜色预览辅助函数
export function hslToCssColor(hsl: string): string {
  return `hsl(${hsl})`
}
