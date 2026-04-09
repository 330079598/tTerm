import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { useToast } from "@/hooks/use-toast"
import type { PresetThemeId } from "@/types/theme"

export interface SettingsDialogProps {
  onClose: () => void
  defaultTab?: string
}

export type ConfigState = ReturnType<typeof useConfig>["config"]
export type SecretStatusState = ReturnType<typeof useConfig>["secretStatus"]
export type SaveConfig = ReturnType<typeof useConfig>["saveConfig"]
export type UpdateLanguage = ReturnType<typeof useConfig>["updateLanguage"]
export type RefreshSecretStatus = ReturnType<typeof useConfig>["refreshSecretStatus"]
export type SetSecretVaultEnabled = ReturnType<typeof useConfig>["setSecretVaultEnabled"]
export type UnlockSecretVault = ReturnType<typeof useConfig>["unlockSecretVault"]
export type LockSecretVault = ReturnType<typeof useConfig>["lockSecretVault"]

export type ThemeContextState = ReturnType<typeof useTheme>
export type ToastState = ReturnType<typeof useToast>["toast"]

export const FONT_SIZE_OPTIONS = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24]

export const PRESET_THEME_COLORS: Record<PresetThemeId, string> = {
  default: "hsl(220 13% 12%)",
  light: "hsl(0 0% 98%)",
  ocean: "hsl(200 30% 10%)",
  forest: "hsl(140 25% 12%)",
  sunset: "hsl(20 30% 12%)",
  ubuntu: "hsl(300 100% 6%)",
}

export const languages = [
  { code: "en", label: "English", nativeLabel: "English" },
  { code: "zh", label: "中文", nativeLabel: "Chinese" },
]
