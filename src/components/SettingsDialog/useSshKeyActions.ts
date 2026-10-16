import { useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { announceVaultKeysChanged } from "@/hooks/useVaultKeys"
import type { VaultKeySummary } from "@/lib/keyVault"
import { isVerificationCanceled } from "@/lib/userVerification"
import { toErrorMessage } from "@/lib/utils"

interface SshKeyActionsOptions {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

/** A file name for an exported key: its name, without characters paths forbid. */
function exportFileName(name: string): string {
  return name.replace(/[\\/:*?"<>|\s]+/g, "_").replace(/^_+|_+$/g, "") || "id_key"
}

/**
 * What the key vault card does with a key: copying its public half, renaming,
 * replacing its certificate, exporting it as a key file and deleting it.
 * `busyId` is the key being worked on.
 */
export function useSshKeyActions({ confirm }: SshKeyActionsOptions) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { withVerification } = useUserVerification()
  const [busyId, setBusyId] = useState<string | null>(null)

  const failed = (caught: unknown) =>
    toast({
      title: t("keyVault.actionFailed"),
      description: toErrorMessage(caught),
      variant: "destructive",
    })

  /** Runs `work` for `key`, reporting a failure; other lists load the vault again after. */
  const run = async (key: VaultKeySummary, work: () => Promise<void>) => {
    setBusyId(key.id)
    try {
      await work()
      announceVaultKeysChanged()
    } catch (caught) {
      if (!isVerificationCanceled(caught)) failed(caught)
    } finally {
      setBusyId(null)
    }
  }

  const copyPublicKey = async (key: VaultKeySummary) => {
    try {
      await invoke("plugin:clipboard-manager|write_text", { text: key.publicKey })
      toast({
        title: t("keyVault.publicKeyCopied"),
        description: t("keyVault.publicKeyCopiedDesc"),
      })
    } catch (caught) {
      failed(caught)
    }
  }

  const renameKey = (key: VaultKeySummary, name: string) =>
    run(key, async () => {
      await invoke("rename_vault_key", { input: { id: key.id, name } })
    })

  const chooseCertificate = async (key: VaultKeySummary) => {
    const selected = await openFileDialog({ multiple: false }).catch(() => null)
    if (!selected || typeof selected !== "string") return
    await run(key, async () => {
      await invoke("set_vault_key_certificate", {
        input: { id: key.id, certificatePath: selected },
      })
      toast({ title: t("keyVault.certificateUpdated") })
    })
  }

  const removeCertificate = (key: VaultKeySummary) =>
    run(key, async () => {
      await invoke("set_vault_key_certificate", { input: { id: key.id, certificatePath: null } })
      toast({ title: t("keyVault.certificateRemoved") })
    })

  const exportKey = async (key: VaultKeySummary) => {
    const path = await saveFileDialog({ defaultPath: exportFileName(key.name) }).catch(() => null)
    if (!path) return
    await run(key, async () => {
      await withVerification("sensitive", () =>
        invoke("export_vault_key", { input: { id: key.id, path } })
      )
      toast({
        title: t("keyVault.exported"),
        description: key.hasCertificate
          ? t("keyVault.exportedWithCertificateDesc", { path })
          : t("keyVault.exportedDesc", { path }),
      })
    })
  }

  const deleteKey = async (key: VaultKeySummary) => {
    const confirmed = await confirm({
      title: t("keyVault.deleteTitle"),
      description:
        key.usedBy.length > 0
          ? t("keyVault.deleteConfirmUsed", { name: key.name, names: key.usedBy.join(", ") })
          : t("keyVault.deleteConfirm", { name: key.name }),
      confirmText: t("common.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return
    await run(key, async () => {
      await invoke("delete_vault_key", { input: { id: key.id } })
      toast({ title: t("keyVault.deleted"), description: key.name })
    })
  }

  return {
    busyId,
    copyPublicKey,
    renameKey,
    chooseCertificate,
    removeCertificate,
    exportKey,
    deleteKey,
  }
}
