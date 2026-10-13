import { Eye, EyeOff, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { getDetectedPlatform } from "@/lib/uiPlatform"
import { useConfig } from "@/contexts/ConfigContext"
import type { SavedSecretEntry, SecretStorageMode } from "@/lib/appConfig"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"

interface SavedSecretsCardProps {
  busy: boolean
  mode: SecretStorageMode
  unlocked: boolean
  handleRevealToggle: (entry: SavedSecretEntry) => Promise<void>
  handleDeleteSavedSecret: (entry: SavedSecretEntry) => Promise<void>
  savedSecrets: SavedSecretEntry[]
  revealedSecretKey: string | null
  revealedPassword: string | null
  revealError: string | null
  secretStatus: ReturnType<typeof useConfig>["secretStatus"]
}

/** The saved passwords: revealing one for a few seconds, or deleting it. */
export function SavedSecretsCard({
  busy,
  mode,
  unlocked,
  handleRevealToggle,
  handleDeleteSavedSecret,
  savedSecrets,
  revealedSecretKey,
  revealedPassword,
  revealError,
  secretStatus,
}: SavedSecretsCardProps) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-sm font-medium">{t("secretStorage.savedPasswords")}</div>
            <div className="text-muted-foreground mt-1 text-xs leading-5">
              {t("secretStorage.savedPasswordsDesc")}
            </div>
            {mode !== "memory" && (
              <div className="text-muted-foreground mt-1 text-xs leading-5">
                {t(
                  secretStatus.verificationMethod === "system" && getDetectedPlatform() === "macos"
                    ? "userVerification.methods.systemMac"
                    : `userVerification.methods.${secretStatus.verificationMethod}`
                )}
              </div>
            )}
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
            {savedSecrets.map((entry) => {
              const revealed = revealedSecretKey === entry.key
              const revealLabel = revealed
                ? t("secretStorage.hideSavedSecret")
                : t("secretStorage.showSavedSecret")
              return (
                <div key={entry.key} className="flex items-center justify-between gap-3 px-3 py-2">
                  <div className="min-w-0">
                    <div className="truncate text-sm">{entry.label}</div>
                    <div className="text-muted-foreground truncate text-xs">
                      {entry.kind.includes("jump")
                        ? t("secretStorage.secretKinds.jumpHost")
                        : entry.kind === "sudo"
                          ? t("secretStorage.secretKinds.sudo")
                          : entry.kind === "proxy"
                            ? t("secretStorage.secretKinds.proxy")
                            : t("secretStorage.secretKinds.ssh")}
                    </div>
                    {revealed && revealedPassword !== null && (
                      <div className="bg-muted/60 mt-1 rounded-sm px-2 py-1 font-mono text-xs break-all">
                        {revealedPassword}
                      </div>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={busy || !unlocked}
                      onClick={() => void handleRevealToggle(entry)}
                      title={revealLabel}
                      aria-label={revealLabel}
                    >
                      {revealed ? <EyeOff size={14} /> : <Eye size={14} />}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      className="text-destructive hover:text-destructive"
                      disabled={busy}
                      onClick={() => void handleDeleteSavedSecret(entry)}
                      title={t("secretStorage.deleteSavedSecret")}
                      aria-label={t("secretStorage.deleteSavedSecret")}
                    >
                      <Trash2 size={14} />
                    </Button>
                  </div>
                </div>
              )
            })}
          </div>
        )}
        {revealError && <p className="text-destructive text-xs leading-5">{revealError}</p>}
      </CardContent>
    </Card>
  )
}
