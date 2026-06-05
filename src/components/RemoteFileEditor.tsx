import "@/components/RemoteFileEditor.css"
import { invoke } from "@tauri-apps/api/core"
import { Loader2, RefreshCcw, Save } from "lucide-react"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { CodeMirrorEditor } from "@/components/CodeMirrorEditor"
import { toast } from "@/hooks/use-toast"
import type { Tab } from "@/types/tab"

interface RemoteFileEditorProps {
  tab: Tab
  onTabUpdate: (updater: (tab: Tab) => Tab) => void
}

interface SftpEditableFile {
  content: string
  fileName: string
  modifiedAt?: number
  path: string
  size: number
}

interface SftpSaveEditedFileResult {
  modifiedAt?: number
  size: number
}

export const RemoteFileEditor: React.FC<RemoteFileEditorProps> = ({ tab, onTabUpdate }) => {
  const { t } = useTranslation()
  const remoteFile = tab.remoteFile
  const [content, setContent] = useState("")
  const [savedContent, setSavedContent] = useState("")
  const [baseline, setBaseline] = useState<{ modifiedAt?: number; size: number } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const loadedPathRef = useRef<string | null>(null)

  const dirty = content !== savedContent
  const connectionLabel =
    remoteFile?.connectionLabel ?? remoteFile?.profileName ?? tab.connection?.profileName
  const subtitle = connectionLabel ? `${connectionLabel} · ${remoteFile?.path}` : remoteFile?.path
  const saveStatus = isSaving
    ? t("remoteFileEditor.saving", { defaultValue: "Saving..." })
    : isLoading
      ? t("remoteFileEditor.loadingShort", { defaultValue: "Loading..." })
      : dirty
        ? t("remoteFileEditor.unsaved", { defaultValue: "Unsaved changes" })
        : t("remoteFileEditor.saved", { defaultValue: "Saved" })
  const saveStatusState = isSaving ? "saving" : dirty ? "dirty" : "saved"

  useEffect(() => {
    if (tab.isModified === dirty) {
      return
    }

    onTabUpdate((current) => ({ ...current, isModified: dirty }))
  }, [dirty, onTabUpdate, tab.isModified])

  const loadFile = useCallback(async () => {
    if (!remoteFile) {
      return
    }

    setIsLoading(true)
    setError(null)
    try {
      const result = await invoke<SftpEditableFile>("sftp_open_file_for_edit", {
        tabId: remoteFile.sourceTabId,
        connection: tab.connection,
        path: remoteFile.path,
      })
      setContent(result.content)
      setSavedContent(result.content)
      setBaseline({ modifiedAt: result.modifiedAt, size: result.size })
      loadedPathRef.current = result.path
      onTabUpdate((current) => ({
        ...current,
        title: result.fileName,
        isModified: false,
        remoteFile: {
          ...(current.remoteFile ?? remoteFile),
          fileName: result.fileName,
          modifiedAt: result.modifiedAt,
          path: result.path,
          size: result.size,
        },
      }))
    } catch (invokeError) {
      const message = String(invokeError)
      setError(message)
      toast({
        variant: "destructive",
        title: t("remoteFileEditor.openFailed", { defaultValue: "Failed to open remote file" }),
        description: message,
      })
    } finally {
      setIsLoading(false)
    }
  }, [onTabUpdate, remoteFile, t, tab.connection])

  useEffect(() => {
    if (!remoteFile || loadedPathRef.current === remoteFile.path) {
      return
    }

    void loadFile()
  }, [loadFile, remoteFile])

  const saveFile = useCallback(async () => {
    if (!remoteFile || !baseline || isSaving) {
      return
    }

    setIsSaving(true)
    setError(null)
    try {
      const result = await invoke<SftpSaveEditedFileResult>("sftp_save_edited_file", {
        tabId: remoteFile.sourceTabId,
        connection: tab.connection,
        path: remoteFile.path,
        content,
        baseline,
      })
      setSavedContent(content)
      setBaseline(result)
      onTabUpdate((current) => ({
        ...current,
        isModified: false,
        remoteFile: current.remoteFile
          ? {
              ...current.remoteFile,
              modifiedAt: result.modifiedAt,
              size: result.size,
            }
          : current.remoteFile,
      }))
      toast({
        title: t("remoteFileEditor.saveSuccess", { defaultValue: "Remote file saved" }),
        description: remoteFile.path,
      })
    } catch (invokeError) {
      const message = String(invokeError)
      setError(message)
      toast({
        variant: "destructive",
        title: t("remoteFileEditor.saveFailed", { defaultValue: "Failed to save remote file" }),
        description: message,
      })
    } finally {
      setIsSaving(false)
    }
  }, [baseline, content, isSaving, onTabUpdate, remoteFile, t, tab.connection])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault()
        void saveFile()
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [saveFile])

  if (!remoteFile) {
    return (
      <div className="remote-file-editor">
        <div className="remote-file-editor-empty">
          {t("remoteFileEditor.missingFile", { defaultValue: "Remote file metadata is missing." })}
        </div>
      </div>
    )
  }

  return (
    <div className="remote-file-editor">
      <div className="remote-file-editor-toolbar">
        <div className="remote-file-editor-title">
          <div className="remote-file-editor-name">
            {dirty ? "*" : ""}
            {remoteFile.fileName}
          </div>
          <div className="remote-file-editor-path" title={subtitle}>
            {subtitle}
          </div>
        </div>
        <div className="remote-file-editor-actions">
          <span className={`remote-file-editor-save-status ${saveStatusState}`} aria-live="polite">
            {saveStatus}
          </span>
          <Button variant="outline" size="sm" onClick={() => void loadFile()} disabled={isLoading}>
            <RefreshCcw className="size-4" />
            {t("remoteFileEditor.reload", { defaultValue: "Reload" })}
          </Button>
          <Button
            size="sm"
            onClick={() => void saveFile()}
            disabled={!dirty || isSaving || isLoading}
          >
            {isSaving ? <Loader2 className="size-4 animate-spin" /> : <Save className="size-4" />}
            {t("remoteFileEditor.save", { defaultValue: "Save" })}
          </Button>
        </div>
      </div>

      {error && (
        <div className="text-destructive border-b px-3 py-2 text-xs" role="alert">
          {error}
        </div>
      )}

      {isLoading ? (
        <div className="remote-file-editor-empty">
          <Loader2 className="mr-2 size-4 animate-spin" />
          {t("remoteFileEditor.loading", { defaultValue: "Loading remote file..." })}
        </div>
      ) : (
        <CodeMirrorEditor
          className="remote-file-editor-code"
          fileName={remoteFile.fileName}
          value={content}
          onChange={setContent}
        />
      )}
    </div>
  )
}
