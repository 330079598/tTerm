import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useTranslation } from "react-i18next"

import {
  EXTERNAL_EDIT_EVENT,
  hasLocalChanges,
  LARGE_EXTERNAL_FILE_BYTES,
  SLOW_EXTERNAL_FILE_BYTES,
  upsertExternalEdit,
  type ExternalEdit,
  type ExternalUploadOutcome,
  type OpenIn,
} from "@/components/SftpDrawer/externalEdit"
import { formatBytes } from "@/components/SftpDrawer/sftpDrawerUtils"
import type { SftpDirectoryEntry, SftpDrawerProps } from "@/components/SftpDrawer/types"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { ToastAction } from "@/components/ui/toast"
import { useConfig } from "@/contexts/ConfigContext"
import { useTransferManager } from "@/contexts/TransferContext"
import { toast } from "@/hooks/use-toast"
import { pickEditorProgram } from "@/lib/externalEditor"
import { toErrorMessage } from "@/lib/utils"

interface UseSftpExternalEditsOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
}

type ToastHandle = ReturnType<typeof toast>

// Saves come often while a file is being edited, so their notes stay small:
// one line, and the uploaded one gone again soon.
const SAVE_TOAST_CLASS = "py-2 pl-3"
// The amber of the files button's count, which marks the same unsent save.
const ASK_TOAST_CLASS = `${SAVE_TOAST_CLASS} border-l-4 border-l-amber-500`
const UPLOADED_TOAST_MS = 2000

/**
 * The tab's remote files open in programs on this computer: opening one,
 * uploading its saved changes (at once or after asking, as the settings
 * say), and letting it go. The backend watches the copies and says when
 * one changes.
 */
export function useSftpExternalEdits({ tabId, connection }: UseSftpExternalEditsOptions) {
  const { t } = useTranslation()
  const { config, saveConfig } = useConfig()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const { addTransfer, updateTransfer } = useTransferManager()
  const [edits, setEdits] = useState<ExternalEdit[]>([])
  // Read from the backend's events, which outlive any one render.
  const latest = useRef({ connection, uploadMode: config.sftp_external_upload })
  const prompts = useRef(new Map<string, ToastHandle>())
  // Files whose upload, or the question it raised, is under way: the state
  // changes it sends back are not new saves to act on.
  const uploading = useRef(new Set<string>())
  // Where each file was opened, to open it there again.
  const openedIn = useRef(new Map<string, OpenIn>())

  useEffect(() => {
    latest.current = { connection, uploadMode: config.sftp_external_upload }
  }, [config.sftp_external_upload, connection])

  const dismissPrompt = useCallback((id: string) => {
    prompts.current.get(id)?.dismiss()
    prompts.current.delete(id)
  }, [])

  const upload = useCallback(
    async (edit: ExternalEdit) => {
      dismissPrompt(edit.id)
      uploading.current.add(edit.id)
      try {
        const run = (overwrite: boolean) =>
          invoke<ExternalUploadOutcome>("sftp_external_edit_upload", {
            tabId,
            connection: latest.current.connection,
            id: edit.id,
            overwrite,
          })
        let outcome = await run(false)
        if (outcome.kind === "remoteChanged") {
          const overwrite = await confirm({
            title: t("sftp.externalEdit.remoteChangedTitle", { name: edit.fileName }),
            description: t("sftp.externalEdit.remoteChangedDescription", {
              path: edit.remotePath,
            }),
            confirmText: t("sftp.externalEdit.overwrite"),
            variant: "destructive",
            defaultAction: "cancel",
          })
          if (!overwrite) return
          outcome = await run(true)
        }
        if (outcome.kind === "uploaded") {
          toast({
            title: t("sftp.externalEdit.uploaded", { name: edit.fileName }),
            variant: "success",
            className: SAVE_TOAST_CLASS,
            duration: UPLOADED_TOAST_MS,
          })
        }
      } catch (error) {
        toast({
          variant: "destructive",
          title: t("sftp.externalEdit.uploadFailed", { name: edit.fileName }),
          description: toErrorMessage(error),
        })
      } finally {
        uploading.current.delete(edit.id)
      }
    },
    [confirm, dismissPrompt, t, tabId]
  )

  const askToUpload = useCallback(
    (edit: ExternalEdit) => {
      dismissPrompt(edit.id)
      const label = t("sftp.externalEdit.upload")
      prompts.current.set(
        edit.id,
        toast({
          title: t("sftp.externalEdit.changedTitle", { name: edit.fileName }),
          className: ASK_TOAST_CLASS,
          duration: Number.POSITIVE_INFINITY,
          action: (
            <ToastAction
              altText={label}
              className="h-7 px-2.5 text-xs"
              onClick={() => void upload(edit)}
            >
              {label}
            </ToastAction>
          ),
        })
      )
    },
    [dismissPrompt, t, upload]
  )

  // Read inside the event listener, so it acts on the latest settings.
  const onChange = useRef<(edit: ExternalEdit) => void>(() => {})
  useEffect(() => {
    onChange.current = (edit) => {
      if (uploading.current.has(edit.id)) return
      if (edit.status !== "modified") {
        if (edit.status === "synced") dismissPrompt(edit.id)
        return
      }
      if (latest.current.uploadMode === "auto") void upload(edit)
      else askToUpload(edit)
    }
  }, [askToUpload, dismissPrompt, upload])

  useEffect(() => {
    let disposed = false
    invoke<ExternalEdit[]>("sftp_external_edit_list", { tabId })
      .then((list) => {
        if (!disposed) setEdits(list)
      })
      .catch(console.error)
    const unlisten = listen<ExternalEdit>(EXTERNAL_EDIT_EVENT, ({ payload }) => {
      if (payload.tabId !== tabId) return
      setEdits((list) => upsertExternalEdit(list, payload))
      onChange.current(payload)
    })
    return () => {
      disposed = true
      void unlisten.then((stop) => stop())
    }
  }, [tabId])

  // The drawer lives as long as its tab: unmounting means the tab closed.
  // Nothing else may run this, so it reads `t` through a ref.
  const translate = useRef(t)
  useEffect(() => {
    translate.current = t
  }, [t])
  useEffect(() => {
    const openPrompts = prompts.current
    return () => {
      openPrompts.forEach((prompt) => prompt.dismiss())
      openPrompts.clear()
      invoke<ExternalEdit[]>("sftp_external_edit_release_tab", { tabId })
        .then((kept) => {
          if (kept.length === 0) return
          toast({
            variant: "destructive",
            title: translate.current("sftp.externalEdit.keptTitle", { count: kept.length }),
            description: kept.map((edit) => edit.localPath).join("\n"),
            duration: Number.POSITIVE_INFINITY,
          })
        })
        .catch(console.error)
    }
  }, [tabId])

  // A large file downloads through the transfer list, where its progress
  // shows and it can be cancelled.
  const open = useCallback(
    async (path: string, fileName: string, openIn: OpenIn, size?: number) => {
      const transferId =
        (size ?? 0) > SLOW_EXTERNAL_FILE_BYTES
          ? addTransfer({
              tabId,
              direction: "download",
              fileName,
              fileSize: size ?? 0,
              localPath: "",
              remotePath: path,
              speed: 0,
            })
          : null
      if (transferId) updateTransfer(transferId, { status: "transferring" })
      try {
        const edit = await invoke<ExternalEdit | null>("sftp_external_edit_open", {
          tabId,
          connection: latest.current.connection,
          path,
          openIn,
          transferId,
        })
        if (transferId) {
          updateTransfer(transferId, {
            status: "completed",
            transferred: size ?? 0,
            localPath: edit?.localPath ?? "",
            endTime: Date.now(),
          })
        }
        if (!edit) return
        openedIn.current.set(edit.id, openIn)
        setEdits((list) => upsertExternalEdit(list, edit))
      } catch (error) {
        const message = toErrorMessage(error)
        const cancelled = message.toLowerCase().includes("cancelled")
        if (transferId) {
          updateTransfer(transferId, {
            status: cancelled ? "cancelled" : "failed",
            error: cancelled ? undefined : message,
            endTime: Date.now(),
          })
        }
        if (cancelled) return
        toast({
          variant: "destructive",
          title: t("sftp.externalEdit.openFailed", { name: fileName }),
          description: message,
        })
      }
    },
    [addTransfer, t, tabId, updateTransfer]
  )

  /** Opens the entry as `openIn` says, asking first for a very large file. */
  const openEntry = useCallback(
    async (entry: SftpDirectoryEntry, openIn: OpenIn) => {
      if (entry.isDir) return
      if ((entry.size ?? 0) > LARGE_EXTERNAL_FILE_BYTES) {
        const confirmed = await confirm({
          title: t("sftp.externalEdit.largeTitle", { name: entry.name }),
          description: t("sftp.externalEdit.largeDescription", {
            size: formatBytes(entry.size),
          }),
          confirmText: t("sftp.externalEdit.openAnyway"),
        })
        if (!confirmed) return
      }
      await open(entry.path, entry.name, openIn, entry.size)
    },
    [confirm, open, t]
  )

  /** Opens the entry in the external editor, or in its default program. */
  const openExternally = useCallback(
    async (entry: SftpDirectoryEntry, inEditor: boolean) => {
      if (!inEditor) {
        await openEntry(entry, { kind: "default" })
        return
      }
      const program =
        config.sftp_external_editor ||
        (await pickEditorProgram(t("settings.sftpExternalEditorChoose")))
      if (!program) return
      if (!config.sftp_external_editor) {
        await saveConfig({ sftp_external_editor: program }).catch(console.error)
      }
      await openEntry(entry, { kind: "program", program })
    },
    [config.sftp_external_editor, openEntry, saveConfig, t]
  )

  const reopen = useCallback(
    (edit: ExternalEdit) =>
      open(edit.remotePath, edit.fileName, openedIn.current.get(edit.id) ?? { kind: "default" }),
    [open]
  )

  const stop = useCallback(
    async (edit: ExternalEdit) => {
      if (hasLocalChanges(edit)) {
        const confirmed = await confirm({
          title: t("sftp.externalEdit.stopTitle", { name: edit.fileName }),
          description: t("sftp.externalEdit.stopDescription"),
          confirmText: t("sftp.externalEdit.discard"),
          variant: "destructive",
          defaultAction: "cancel",
        })
        if (!confirmed) return
      }
      dismissPrompt(edit.id)
      openedIn.current.delete(edit.id)
      try {
        await invoke("sftp_external_edit_stop", { id: edit.id })
        setEdits((list) => list.filter((existing) => existing.id !== edit.id))
      } catch (error) {
        toast({ variant: "destructive", title: toErrorMessage(error) })
      }
    },
    [confirm, dismissPrompt, t]
  )

  const reveal = useCallback(async (edit: ExternalEdit) => {
    try {
      await invoke("sftp_external_edit_reveal", { id: edit.id })
    } catch (error) {
      toast({ variant: "destructive", title: toErrorMessage(error) })
    }
  }, [])

  return { edits, openEntry, openExternally, upload, reopen, stop, reveal, ConfirmDialog }
}
