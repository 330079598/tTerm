import React, { useMemo, useState } from "react"
import { Copy, Download, FileBadge, FileX, KeyRound, Pencil, Plus, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ImportSshKeyDialog } from "@/components/SettingsDialog/ImportSshKeyDialog"
import { useSshKeyActions } from "@/components/SettingsDialog/useSshKeyActions"
import { RenameDialog } from "@/components/RenameDialog"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { useVaultKeys } from "@/hooks/useVaultKeys"
import type { SecretStorageMode } from "@/lib/appConfig"
import { shortFingerprint, type VaultKeySummary } from "@/lib/keyVault"

interface SshKeysCardProps {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  mode: SecretStorageMode
  unlocked: boolean
}

/** The SSH key vault: private keys kept encrypted with the saved passwords. */
export const SshKeysCard: React.FC<SshKeysCardProps> = ({ confirm, mode, unlocked }) => {
  const { t } = useTranslation()
  const { keys, loaded, loadError } = useVaultKeys()
  const actions = useSshKeyActions({ confirm })
  const [importing, setImporting] = useState(false)
  const [renaming, setRenaming] = useState<VaultKeySummary | null>(null)
  const usable = mode !== "memory" && unlocked

  const notice =
    mode === "memory"
      ? t("keyVault.unavailableInMemory")
      : !unlocked
        ? t("keyVault.locked")
        : loadError
          ? t("keyVault.loadFailed", { error: loadError })
          : loaded && keys.length === 0
            ? t("keyVault.empty")
            : null

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="text-sm font-medium">{t("keyVault.title")}</div>
            <div className="text-muted-foreground mt-1 text-xs leading-5">
              {t("keyVault.description")}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Badge variant="secondary">{t("keyVault.count", { count: keys.length })}</Badge>
            <Button
              size="sm"
              variant="outline"
              disabled={!usable}
              onClick={() => setImporting(true)}
            >
              <Plus size={14} />
              {t("keyVault.import")}
            </Button>
          </div>
        </div>

        {notice && (
          <p
            className={
              loadError && usable
                ? "text-destructive text-xs"
                : "text-muted-foreground text-xs leading-5"
            }
          >
            {notice}
          </p>
        )}

        {usable && keys.length > 0 && (
          <div className="border-border divide-border divide-y overflow-hidden rounded-md border">
            {keys.map((key) => (
              <SshKeyRow
                key={key.id}
                vaultKey={key}
                busy={actions.busyId !== null}
                actions={actions}
                onRename={() => setRenaming(key)}
              />
            ))}
          </div>
        )}
      </CardContent>

      <ImportSshKeyDialog open={importing} onOpenChange={setImporting} />
      <RenameDialog
        isOpen={renaming !== null}
        currentName={renaming?.name ?? ""}
        onConfirm={(name) => {
          if (renaming) void actions.renameKey(renaming, name)
        }}
        onClose={() => setRenaming(null)}
      />
    </Card>
  )
}

interface SshKeyRowProps {
  vaultKey: VaultKeySummary
  busy: boolean
  actions: ReturnType<typeof useSshKeyActions>
  onRename: () => void
}

const SshKeyRow: React.FC<SshKeyRowProps> = ({ vaultKey: key, busy, actions, onRename }) => {
  const { t } = useTranslation()
  const dateFormat = useMemo(() => new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }), [])

  const details = [
    key.algorithm,
    key.encrypted &&
      (key.passphraseSaved ? t("keyVault.passphraseSaved") : t("keyVault.encrypted")),
    key.hasCertificate &&
      (key.certificateValidBefore === null
        ? t("keyVault.certificateForever")
        : t("keyVault.certificateValidUntil", {
            date: dateFormat.format(key.certificateValidBefore * 1000),
          })),
  ].filter(Boolean)

  const buttons: { label: string; icon: React.ReactNode; onClick: () => void; danger?: boolean }[] =
    [
      {
        label: t("keyVault.copyPublicKey"),
        icon: <Copy size={14} />,
        onClick: () => void actions.copyPublicKey(key),
      },
      { label: t("keyVault.rename"), icon: <Pencil size={14} />, onClick: onRename },
      {
        label: key.hasCertificate ? t("keyVault.replaceCertificate") : t("keyVault.addCertificate"),
        icon: <FileBadge size={14} />,
        onClick: () => void actions.chooseCertificate(key),
      },
      ...(key.hasCertificate
        ? [
            {
              label: t("keyVault.removeCertificate"),
              icon: <FileX size={14} />,
              onClick: () => void actions.removeCertificate(key),
            },
          ]
        : []),
      {
        label: t("keyVault.export"),
        icon: <Download size={14} />,
        onClick: () => void actions.exportKey(key),
      },
      {
        label: t("keyVault.delete"),
        icon: <Trash2 size={14} />,
        onClick: () => void actions.deleteKey(key),
        danger: true,
      },
    ]

  return (
    <div className="flex items-start justify-between gap-3 px-3 py-2">
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <KeyRound size={14} className="text-muted-foreground shrink-0" />
          <span className="truncate text-sm">{key.name}</span>
        </div>
        <div className="text-muted-foreground truncate text-xs">{details.join(" · ")}</div>
        <div
          className="text-muted-foreground mt-0.5 font-mono text-xs break-all"
          title={key.fingerprint}
        >
          {shortFingerprint(key.fingerprint)}
        </div>
        <div className="text-muted-foreground mt-0.5 truncate text-xs">
          {key.usedBy.length > 0
            ? t("keyVault.usedBy", { names: key.usedBy.join(", ") })
            : t("keyVault.notUsed")}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-0.5">
        {buttons.map((button) => (
          <Button
            key={button.label}
            variant="ghost"
            size="icon-sm"
            className={button.danger ? "text-destructive hover:text-destructive" : undefined}
            disabled={busy}
            onClick={button.onClick}
            title={button.label}
            aria-label={`${button.label}: ${key.name}`}
          >
            {button.icon}
          </Button>
        ))}
      </div>
    </div>
  )
}
