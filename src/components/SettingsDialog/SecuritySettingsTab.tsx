import React, { useEffect, useRef, useState } from "react"
import { AlertTriangle, ArrowLeftRight, Eye, EyeOff, Lock, Trash2, Unlock } from "lucide-react"
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
  handleChangePassword: () => Promise<void>
  handleDeleteSavedSecret: (entry: SavedSecretEntry) => Promise<void>
  handleRevealSavedSecret: (key: string) => Promise<string>
  handlePromptUnlockOnStartupChange: (checked: boolean) => Promise<void>
  handleSecretStorageModeChange: (mode: SecretStorageMode) => Promise<void>
  handleUnlock: () => Promise<void>
  password: string
  currentPassword: string
  newPassword: string
  confirmPassword: string
  promptUnlockVaultOnStartup: boolean
  savedSecrets: SavedSecretEntry[]
  secretBusy: boolean
  secretError: string | null
  secretStatus: SecretStatusState
  secretStorageMode: SecretStorageMode
  setPassword: React.Dispatch<React.SetStateAction<string>>
  setCurrentPassword: React.Dispatch<React.SetStateAction<string>>
  setNewPassword: React.Dispatch<React.SetStateAction<string>>
  setConfirmPassword: React.Dispatch<React.SetStateAction<string>>
}

export const SecuritySettingsTab: React.FC<SecuritySettingsTabProps> = ({
  backendLabel,
  configSecretVaultEnabled,
  handleCopySecretStore,
  handleEnableVault,
  handleLock,
  handleChangePassword,
  handleDeleteSavedSecret,
  handleRevealSavedSecret,
  handlePromptUnlockOnStartupChange,
  handleSecretStorageModeChange,
  handleUnlock,
  password,
  currentPassword,
  newPassword,
  confirmPassword,
  promptUnlockVaultOnStartup,
  savedSecrets,
  secretBusy,
  secretError,
  secretStatus,
  secretStorageMode,
  setPassword,
  setCurrentPassword,
  setNewPassword,
  setConfirmPassword,
}) => {
  const { t } = useTranslation()
  const [showVaultPassword, setShowVaultPassword] = useState(false)
  const [showCurrentPassword, setShowCurrentPassword] = useState(false)
  const [showNewPassword, setShowNewPassword] = useState(false)
  const [showConfirmPassword, setShowConfirmPassword] = useState(false)
  const [revealedSecretKey, setRevealedSecretKey] = useState<string | null>(null)
  const [revealedPassword, setRevealedPassword] = useState<string | null>(null)
  const [revealError, setRevealError] = useState(false)
  const revealTimerRef = useRef<number | null>(null)
  const revealRequestRef = useRef(0)
  const vaultControlsVisible =
    secretStorageMode === "vault" || secretStorageMode === "auto" || secretStorageMode === "hybrid"
  const vaultSettingsVisible = vaultControlsVisible && configSecretVaultEnabled
  const isHybrid = secretStorageMode === "hybrid"

  const clearRevealedPassword = () => {
    if (revealTimerRef.current !== null) {
      window.clearTimeout(revealTimerRef.current)
      revealTimerRef.current = null
    }
    setRevealedSecretKey(null)
    setRevealedPassword(null)
  }

  useEffect(
    () => () => {
      if (revealTimerRef.current !== null) {
        window.clearTimeout(revealTimerRef.current)
      }
    },
    []
  )

  const handleRevealToggle = async (entry: SavedSecretEntry) => {
    if (revealedSecretKey === entry.key) {
      clearRevealedPassword()
      return
    }

    const requestId = revealRequestRef.current + 1
    revealRequestRef.current = requestId
    setRevealError(false)
    clearRevealedPassword()

    try {
      const password = await handleRevealSavedSecret(entry.key)
      if (revealRequestRef.current !== requestId) {
        return
      }
      setRevealedSecretKey(entry.key)
      setRevealedPassword(password)
      revealTimerRef.current = window.setTimeout(clearRevealedPassword, 10_000)
    } catch {
      if (revealRequestRef.current === requestId) {
        setRevealError(true)
      }
    }
  }

  const handleLockAndClear = async () => {
    clearRevealedPassword()
    await handleLock()
  }

  const handleDeleteAndClear = async (entry: SavedSecretEntry) => {
    await handleDeleteSavedSecret(entry)
    if (revealedSecretKey === entry.key) {
      clearRevealedPassword()
    }
  }

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
                <option value="hybrid">{t("secretStorage.modes.hybrid")}</option>
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

        {vaultControlsVisible && !isHybrid && (
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

        {isHybrid && !secretStatus.vaultUnlocked && secretStatus.keyringAvailable && (
          <Card>
            <CardContent className="p-4">
              <p className="text-muted-foreground text-xs leading-5">
                {t("secretStorage.hybridSetup")}
              </p>
            </CardContent>
          </Card>
        )}

        {vaultSettingsVisible && (!isHybrid || !secretStatus.vaultUnlocked) && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <Label htmlFor="vault-password" className="mb-1.5 block">
                  {t("secretStorage.vaultPassword")}
                </Label>
                <div className="relative">
                  <Input
                    id="vault-password"
                    type={showVaultPassword ? "text" : "password"}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder={t("secretStorage.vaultPasswordPlaceholder")}
                    disabled={secretBusy}
                    className="pr-9"
                  />
                  <button
                    type="button"
                    onClick={() => setShowVaultPassword((v) => !v)}
                    className="text-muted-foreground hover:text-foreground absolute top-1/2 right-2 -translate-y-1/2"
                    tabIndex={-1}
                  >
                    {showVaultPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </div>
              <div className="flex gap-2">
                {!secretStatus.vaultUnlocked ? (
                  <Button onClick={handleUnlock} disabled={secretBusy || password.length === 0}>
                    <Unlock size={14} className="mr-2" />
                    {isHybrid
                      ? t("secretStorage.setVaultPassword")
                      : t("secretStorage.unlockVault")}
                  </Button>
                ) : (
                  <>
                    <Button variant="outline" onClick={handleLockAndClear} disabled={secretBusy}>
                      <Lock size={14} className="mr-2" />
                      {t("secretStorage.lockVault")}
                    </Button>
                    {isHybrid && (
                      <p className="text-muted-foreground self-center text-xs">
                        {t("secretStorage.lockVaultHybridDesc")}
                      </p>
                    )}
                  </>
                )}
              </div>
              {secretError && !secretStatus.vaultUnlocked && (
                <p className="text-destructive mt-1 text-xs" role="alert">
                  {secretError}
                </p>
              )}
            </CardContent>
          </Card>
        )}

        {vaultSettingsVisible && secretStatus.vaultUnlocked && (
          <Card>
            <CardContent className="space-y-3 p-4">
              <div>
                <div className="text-sm font-medium">{t("secretStorage.changeVaultPassword")}</div>
                <div className="text-muted-foreground mt-1 text-xs leading-5">
                  {t("secretStorage.changeVaultPasswordDesc")}
                </div>
              </div>
              <div className="space-y-2">
                <div>
                  <Label htmlFor="current-password" className="mb-1.5 block">
                    {t("secretStorage.currentPassword")}
                  </Label>
                  <div className="relative">
                    <Input
                      id="current-password"
                      type={showCurrentPassword ? "text" : "password"}
                      value={currentPassword}
                      onChange={(e) => setCurrentPassword(e.target.value)}
                      placeholder={t("secretStorage.vaultPasswordPlaceholder")}
                      disabled={secretBusy}
                      className="pr-9"
                    />
                    <button
                      type="button"
                      onClick={() => setShowCurrentPassword((v) => !v)}
                      className="text-muted-foreground hover:text-foreground absolute top-1/2 right-2 -translate-y-1/2"
                      tabIndex={-1}
                    >
                      {showCurrentPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  </div>
                </div>
                <div>
                  <Label htmlFor="new-password" className="mb-1.5 block">
                    {t("secretStorage.newPassword")}
                  </Label>
                  <div className="relative">
                    <Input
                      id="new-password"
                      type={showNewPassword ? "text" : "password"}
                      value={newPassword}
                      onChange={(e) => setNewPassword(e.target.value)}
                      placeholder={t("secretStorage.vaultPasswordPlaceholder")}
                      disabled={secretBusy}
                      className="pr-9"
                    />
                    <button
                      type="button"
                      onClick={() => setShowNewPassword((v) => !v)}
                      className="text-muted-foreground hover:text-foreground absolute top-1/2 right-2 -translate-y-1/2"
                      tabIndex={-1}
                    >
                      {showNewPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  </div>
                </div>
                <div>
                  <Label htmlFor="confirm-password" className="mb-1.5 block">
                    {t("secretStorage.confirmNewPassword")}
                  </Label>
                  <div className="relative">
                    <Input
                      id="confirm-password"
                      type={showConfirmPassword ? "text" : "password"}
                      value={confirmPassword}
                      onChange={(e) => setConfirmPassword(e.target.value)}
                      placeholder={t("secretStorage.vaultPasswordPlaceholder")}
                      disabled={secretBusy}
                      className="pr-9"
                    />
                    <button
                      type="button"
                      onClick={() => setShowConfirmPassword((v) => !v)}
                      className="text-muted-foreground hover:text-foreground absolute top-1/2 right-2 -translate-y-1/2"
                      tabIndex={-1}
                    >
                      {showConfirmPassword ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  </div>
                </div>
              </div>
              <Button
                onClick={handleChangePassword}
                disabled={
                  secretBusy ||
                  currentPassword.length === 0 ||
                  newPassword.length === 0 ||
                  confirmPassword.length === 0
                }
              >
                <Lock size={14} className="mr-2" />
                {t("secretStorage.changeVaultPassword")}
              </Button>
            </CardContent>
          </Card>
        )}

        {vaultSettingsVisible && !isHybrid && (
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
                      {revealedSecretKey === entry.key && revealedPassword !== null && (
                        <div className="bg-muted/60 mt-1 rounded-sm px-2 py-1 font-mono text-xs break-all">
                          {revealedPassword}
                        </div>
                      )}
                    </div>
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        disabled={secretBusy}
                        onClick={() => void handleRevealToggle(entry)}
                        title={
                          revealedSecretKey === entry.key
                            ? t("secretStorage.hideSavedSecret")
                            : t("secretStorage.showSavedSecret")
                        }
                        aria-label={
                          revealedSecretKey === entry.key
                            ? t("secretStorage.hideSavedSecret")
                            : t("secretStorage.showSavedSecret")
                        }
                      >
                        {revealedSecretKey === entry.key ? <EyeOff size={14} /> : <Eye size={14} />}
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        className="text-destructive hover:text-destructive"
                        disabled={secretBusy}
                        onClick={() => void handleDeleteAndClear(entry)}
                        title={t("secretStorage.deleteSavedSecret")}
                        aria-label={t("secretStorage.deleteSavedSecret")}
                      >
                        <Trash2 size={14} />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
            {revealError && (
              <p className="text-destructive text-xs leading-5">
                {t("secretStorage.revealSavedSecretFailed")}
              </p>
            )}
          </CardContent>
        </Card>

        {secretError && secretStatus.vaultUnlocked && (
          <Alert className="border-destructive/40 bg-destructive/10 text-destructive">
            <AlertTitle>{t("secretStorage.title")}</AlertTitle>
            <AlertDescription className="mt-1 text-sm text-current">{secretError}</AlertDescription>
          </Alert>
        )}
      </div>
    </ScrollArea>
  )
}
