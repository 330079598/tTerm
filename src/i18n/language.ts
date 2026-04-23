export type SupportedLanguage = "en" | "zh"

export function normalizeLanguage(locale?: string | null): SupportedLanguage {
  if (!locale) {
    return "en"
  }

  const normalizedLocale = locale.toLowerCase().replace(/_/g, "-")

  if (normalizedLocale.startsWith("zh")) {
    return "zh"
  }

  return "en"
}

export function detectSystemLanguage(): SupportedLanguage {
  if (typeof navigator === "undefined") {
    return "en"
  }

  return normalizeLanguage(navigator.languages?.[0] ?? navigator.language)
}
