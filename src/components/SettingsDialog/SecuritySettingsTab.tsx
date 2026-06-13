import React from "react"
import { AlertTriangle, ArrowLeftRight, Lock, Trash2, Unlock } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"

import { SecretStatusState } from "@/components/SettingsDialog/types"
import type { SavedSecretEntry, SecretStorageMode } from "@/contexts/ConfigContext"

interface SecuritySettingsTabProps {
  backendLabel: string
  configSecretVaultEnabled: boolean
  handleCopySecretStore: (direction: "systemToVault" | "vaultToSystem") => Promise<void>
  handleEnableVault: (checked: boolean) => Promise<void>
  handleLock: () => Promise<void>
  handleDeleteSavedSecret: (entry: SavedSecretEntry) => Promise<void>
  handlePromptUnlockOnStartupChange: (checked: boolean) => Promise<void>
  handleSecretStorageModeChange: (mode: SecretStorageMode) => Promise<void>
  handleUnlock: () => Promise<void>
  password: string
  promptUnlockVaultOnStartup: boolean
  savedSecrets: SavedSecretEntry[]
  secretBusy: boolean
  secretError: string | null
  secretStatus: SecretStatusState
  secretStorageMode: SecretStorageMode
  setPassword: React.Dispatch<React.SetStateAction<string>>
}

export const SecuritySettingsTab: React.FC<SecuritySettingsTabProps> = ({
  backendLabel,
  configSecretVaultEnabled,
  handleCopySecretStore,
  handleEnableVault,
  handleLock,
  handleDeleteSavedSecret,
  handlePromptUnlockOnStartupChange,
  handleSecretStorageModeChange,
  handleUnlock,
  password,
  promptUnlockVaultOnStartup,
  savedSecrets,
  secretBusy,
  secretError,
  secretStatus,
  secretStorageMode,
  setPassword,
}) => {
  const { t } = useTranslation()
  const vaultControlsVisible = secretStorageMode === "vault" || secretStorageMode === "auto"
  const vaultSettingsVisible = vaultControlsVisible && configSecretVaultEnabled

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-4">
        <p className="text-muted-foreground text-sm">{t("secretStorage.description")}</p>

        <Card>
          <CardContent className="space-y-2 p-4 text-sm">
            <div className="flex items-center justify-between gap-3">
              <span>{t("secretStorage.activeBackend")}</span>
              <Badge variant="secondary">{backendLabel}</Badge>
            </div>
            {secretStatus.message && (
              <p className="text-muted-foreground text-xs leading-5">{secretStatus.message}</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="space-y-3 p-4">
            <div>
              <Label htmlFor="secret-storage-mode" className="mb-1.5 block">
                {t("secretStorage.storageMode")}
              </Label>
              <Select
                id="secret-storage-mode"
                value={secretStorageMode}
                disabled={secretBusy}
                onChange={(event) =>
                  handleSecretStorageModeChange(event.target.value as SecretStorageMode)
                }
              >
                <option value="auto">{t("secretStorage.modes.auto")}</option>
                <option value="system">{t("secretStorage.modes.system")}</option>
                <option value="vault">{t("secretStorage.modes.vault")}</option>
                <option value="memory">{t("secretStorage.modes.memory")}</option>
              </Select>
            </div>
            <p className="text-muted-foreground text-xs leading-5">
              {t(`secretStorage.modeDescriptions.${secretStorageMode}`)}
            </p>
          </CardContent>
        </Card>

        {!secretStatus.keyringAvailable && (
          <Alert className="border-amber-500/40 bg-amber-500/10">
            <div className="flex items-start gap-2">
              <AlertTriangle size={16} className="mt-0.5 shrink-0" />
              <div>
                <AlertTitle>{t("secretStorage.keyringUnavailable")}</AlertTitle>
                <AlertDescription className="mt-1 text-xs leading-5">
                  {t("secretStorage.keyringUnavailableDesc")}
                </AlertDescription>
              </div>
            </div>
          </Alert>
        )}

        {vaultControlsVisible && (
          <Card>
            <CardContent className="flex items-center justify-between gap-4 p-4">
              <div>
                <div className="text-sm font-medium">{t("secretStorage.enableVault")}</div>
                <div className="text-muted-foreground mt-1 text-xs">
                  {t("secretStorage.enableVaultDesc")}
                </div>
              </div>
              <Switch
                checked={configSecretVaultEnabled}
                disabled={secretBusy || secretStorageMode === "vault"}
                onCheckedChange={handleEnableVault}
              />
            </CardContent>
          </Card>
        )}

        {vaultSettingsVisible && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <Label htmlFor="vault-password" className="mb-1.5 block">
                  {t("secretStorage.vaultPassword")}
                </Label>
                <Input
                  id="vault-password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={t("secretStorage.vaultPasswordPlaceholder")}
                  disabled={secretBusy}
                />
              </div>
              <div className="flex gap-2">
                {!secretStatus.vaultUnlocked ? (
                  <Button onClick={handleUnlock} disabled={secretBusy || password.length === 0}>
                    <Unlock size={14} className="mr-2" />
                    {t("secretStorage.unlockVault")}
                  </Button>
                ) : (
                  <Button variant="outline" onClick={handleLock} disabled={secretBusy}>
                    <Lock size={14} className="mr-2" />
                    {t("secretStorage.lockVault")}
                  </Button>
                )}
              </div>
            </CardContent>
          </Card>
        )}

        {vaultSettingsVisible && (
          <Card>
            <CardContent className="flex items-center justify-between gap-4 p-4">
              <div>
                <div className="text-sm font-medium">
                  {t("secretStorage.promptUnlockOnStartup")}
                </div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {t("secretStorage.promptUnlockOnStartupDesc")}
                </div>
              </div>
              <Switch
                checked={promptUnlockVaultOnStartup}
                disabled={secretBusy}
                onCheckedChange={handlePromptUnlockOnStartupChange}
              />
            </CardContent>
          </Card>
        )}

        {vaultSettingsVisible && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <div className="text-sm font-medium">{t("secretStorage.copyTitle")}</div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {t("secretStorage.copyDesc")}
                </div>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  onClick={() => handleCopySecretStore("systemToVault")}
                  disabled={
                    secretBusy || !secretStatus.keyringAvailable || !secretStatus.vaultUnlocked
                  }
                >
                  <ArrowLeftRight size={14} className="mr-2" />
                  {t("secretStorage.copySystemToVault")}
                </Button>
                <Button
                  variant="outline"
                  onClick={() => handleCopySecretStore("vaultToSystem")}
                  disabled={
                    secretBusy || !secretStatus.keyringAvailable || !secretStatus.vaultUnlocked
                  }
                >
                  <ArrowLeftRight size={14} className="mr-2" />
                  {t("secretStorage.copyVaultToSystem")}
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        <Card>
          <CardContent className="space-y-3 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-sm font-medium">{t("secretStorage.savedPasswords")}</div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {t("secretStorage.savedPasswordsDesc")}
                </div>
              </div>
              <Badge variant="secondary">
                {t("secretStorage.savedPasswordCount", { count: savedSecrets.length })}
              </Badge>
            </div>

            {savedSecrets.length === 0 ? (
              <p className="text-muted-foreground text-xs leading-5">
                {t("secretStorage.noSavedPasswords")}
              </p>
            ) : (
              <div className="border-border divide-border overflow-hidden rounded-md border">
                {savedSecrets.map((entry) => (
                  <div
                    key={entry.key}
                    className="flex items-center justify-between gap-3 px-3 py-2"
                  >
                    <div className="min-w-0">
                      <div className="truncate text-sm">{entry.label}</div>
                      <div className="text-muted-foreground truncate text-xs">
                        {entry.kind.includes("jump")
                          ? t("secretStorage.secretKinds.jumpHost")
                          : t("secretStorage.secretKinds.ssh")}
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="text-destructive hover:text-destructive shrink-0"
                      disabled={secretBusy}
                      onClick={() => handleDeleteSavedSecret(entry)}
                      title={t("secretStorage.deleteSavedSecret")}
                    >
                      <Trash2 size={14} />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        {secretError && (
          <Alert className="border-destructive/40 bg-destructive/10 text-destructive">
            <AlertTitle>{t("secretStorage.title")}</AlertTitle>
            <AlertDescription className="mt-1 text-sm text-current">{secretError}</AlertDescription>
          </Alert>
        )}
      </div>
    </ScrollArea>
  )
}
