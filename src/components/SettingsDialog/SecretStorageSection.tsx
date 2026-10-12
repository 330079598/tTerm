import { Shield } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { useConfig } from "@/contexts/ConfigContext"
import type { SecretStorageMode } from "@/lib/appConfig"

interface SecretStorageSectionProps {
  busy: boolean
  mode: SecretStorageMode
  unlocked: boolean
  pending: boolean
  handleModeChange: (next: SecretStorageMode) => Promise<void>
  secretStatus: ReturnType<typeof useConfig>["secretStatus"]
  choosingPasswordMode: boolean
  statusLabel: string
}

/** Where saved passwords are kept, and whether they are unlocked now. */
export function SecretStorageSection({
  busy,
  mode,
  unlocked,
  pending,
  handleModeChange,
  secretStatus,
  choosingPasswordMode,
  statusLabel,
}: SecretStorageSectionProps) {
  const { t } = useTranslation()

  return (
    <SettingsSection
      icon={<Shield size={16} />}
      title={t("secretStorage.title")}
      description={t("secretStorage.description")}
    >
      <SettingsRow
        title={t("secretStorage.status")}
        description={secretStatus.message}
        action={
          <Badge variant={unlocked && mode !== "memory" ? "default" : "secondary"}>
            {statusLabel}
          </Badge>
        }
      />
      <SettingsRow
        title={t("secretStorage.storageMode")}
        description={t(`secretStorage.modeDescriptions.${mode}`)}
      >
        <Label htmlFor="secret-storage-mode" className="sr-only">
          {t("secretStorage.storageMode")}
        </Label>
        <Select
          id="secret-storage-mode"
          value={choosingPasswordMode ? "password" : mode}
          disabled={busy || pending}
          onChange={(event) => void handleModeChange(event.target.value as SecretStorageMode)}
        >
          <option value="system" disabled={!secretStatus.keyringAvailable}>
            {t("secretStorage.modes.system")}
          </option>
          <option value="password">{t("secretStorage.modes.password")}</option>
          <option value="memory">{t("secretStorage.modes.memory")}</option>
        </Select>
      </SettingsRow>
    </SettingsSection>
  )
}
