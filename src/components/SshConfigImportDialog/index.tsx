import React, { useEffect, useEffectEvent, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { FileSearch, FileUp, FolderOpen, Import, Loader2, RefreshCcw } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SshConfigImportHostRow } from "@/components/SshConfigImportDialog/SshConfigImportHostRow"
import type {
  SshConfigImportPreview,
  SshConfigImportResult,
} from "@/components/SshConfigImportDialog/sshConfigImport"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { ScrollArea } from "@/components/ui/scroll-area"
import { toast } from "@/hooks/use-toast"
import type { SavedProfile } from "@/types/tab"

interface SshConfigImportDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onImported: (profiles: SavedProfile[]) => void
}

export const SshConfigImportDialog: React.FC<SshConfigImportDialogProps> = ({
  open,
  onOpenChange,
  onImported,
}) => {
  const { t } = useTranslation()
  const defaultGroup = t("sshConfigImport.defaultGroup", {
    defaultValue: "Imported from SSH config",
  })
  const [sourcePath, setSourcePath] = useState("~/.ssh")
  const [group, setGroup] = useState(defaultGroup)
  const [overwriteExisting, setOverwriteExisting] = useState(false)
  const [importForwards, setImportForwards] = useState(true)
  const [preview, setPreview] = useState<SshConfigImportPreview | null>(null)
  const [selectedHosts, setSelectedHosts] = useState<Set<string>>(new Set())
  const [loadingPreview, setLoadingPreview] = useState(false)
  const [importing, setImporting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const selectableHosts = useMemo(
    () => preview?.hosts.filter((host) => !host.skipped) ?? [],
    [preview]
  )
  const selectedCount = selectedHosts.size
  const forwardCount = useMemo(
    () =>
      selectableHosts
        .filter((host) => selectedHosts.has(host.hostPattern))
        .reduce((total, host) => total + host.forwards.length, 0),
    [selectableHosts, selectedHosts]
  )
  const readyCount = selectableHosts.length

  const formatImportError = (err: unknown) => {
    const message = String(err)
    const lowerMessage = message.toLowerCase()

    if (lowerMessage.includes("not found")) {
      return t("sshConfigImport.errorNotFound")
    }

    if (lowerMessage.includes("permission denied") || lowerMessage.includes("access is denied")) {
      return t("sshConfigImport.errorPermissionDenied")
    }

    if (lowerMessage.includes("parse")) {
      return t("sshConfigImport.errorParse")
    }

    if (message.startsWith("SSH config path is not a file")) {
      return t("sshConfigImport.errorNotFile")
    }

    return message
  }

  const loadPreview = (path: string) => {
    setLoadingPreview(true)
    setError(null)
    return fetchPreview(path)
  }

  const fetchPreview = async (path: string) => {
    try {
      const result = await invoke<SshConfigImportPreview>("preview_ssh_config_import", {
        sourcePath: path.trim() || undefined,
      })
      setPreview(result)
      setSourcePath(result.sourcePath)
      setSelectedHosts(
        new Set(result.hosts.filter((host) => !host.skipped).map((host) => host.hostPattern))
      )
    } catch (err) {
      setPreview(null)
      setSelectedHosts(new Set())
      setError(formatImportError(err))
    } finally {
      setLoadingPreview(false)
    }
  }

  const [previewOpen, setPreviewOpen] = useState(false)
  if (previewOpen !== open) {
    setPreviewOpen(open)
    if (open) {
      setLoadingPreview(true)
      setError(null)
    }
  }

  // Run only when the dialog opens; manual path edits are previewed by the Preview button.
  const previewOnOpen = useEffectEvent(() => void fetchPreview(sourcePath))
  useEffect(() => {
    if (open) previewOnOpen()
  }, [open])

  const chooseSourceDirectory = async () => {
    const selected = await openFileDialog({
      multiple: false,
      directory: true,
      title: t("sshConfigImport.chooseSourceDirectory"),
    }).catch(() => null)

    if (selected && typeof selected === "string") {
      setSourcePath(selected)
      void loadPreview(selected)
    }
  }

  const chooseSourceFile = async () => {
    const selected = await openFileDialog({
      multiple: false,
      directory: false,
      title: t("sshConfigImport.chooseSourceFile"),
    }).catch(() => null)

    if (selected && typeof selected === "string") {
      setSourcePath(selected)
      void loadPreview(selected)
    }
  }

  const toggleHost = (hostPattern: string, checked: boolean) => {
    setSelectedHosts((current) => {
      const next = new Set(current)
      if (checked) {
        next.add(hostPattern)
      } else {
        next.delete(hostPattern)
      }
      return next
    })
  }

  const toggleAll = (checked: boolean) => {
    setSelectedHosts(checked ? new Set(selectableHosts.map((host) => host.hostPattern)) : new Set())
  }

  const handleImport = async () => {
    setImporting(true)
    setError(null)

    try {
      const result = await invoke<SshConfigImportResult>("import_ssh_config_profiles", {
        options: {
          sourcePath,
          group,
          overwriteExisting,
          selectedHosts: Array.from(selectedHosts),
          importForwards,
        },
      })
      onImported(result.profiles)
      toast({
        title: t("sshConfigImport.importComplete"),
        description:
          t("sshConfigImport.importCompleteDesc", {
            imported: result.imported,
            updated: result.updated,
            skipped: result.skipped,
          }) +
          (result.tunnelsImported > 0
            ? " " +
              t("sshConfigImport.importCompleteTunnels", {
                count: result.tunnelsImported,
                defaultValue: "Added {{count}} port forwards.",
              })
            : ""),
      })
      onOpenChange(false)
    } catch (err) {
      setError(formatImportError(err))
    } finally {
      setImporting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[88vh] gap-0 overflow-hidden p-0 sm:max-w-4xl">
        <DialogHeader className="border-border/80 border-b px-6 py-5">
          <div className="flex items-start gap-3">
            <div className="bg-muted text-muted-foreground flex size-9 shrink-0 items-center justify-center rounded-lg border">
              <FileSearch size={17} />
            </div>
            <div className="min-w-0">
              <DialogTitle>{t("sshConfigImport.title")}</DialogTitle>
              <DialogDescription className="mt-1">
                {t("sshConfigImport.description")}
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div className="grid gap-4 px-6 py-5">
          <div className="grid gap-3 lg:grid-cols-[1fr_220px]">
            <div>
              <Label htmlFor="ssh-config-source" className="mb-1.5 block">
                {t("sshConfigImport.source")}
              </Label>
              <div className="flex gap-2">
                <Input
                  id="ssh-config-source"
                  value={sourcePath}
                  onChange={(event) => setSourcePath(event.target.value)}
                  placeholder="~/.ssh"
                  className="font-mono text-xs"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={chooseSourceDirectory}
                  aria-label={t("sshConfigImport.chooseSourceDirectory")}
                >
                  <FolderOpen size={16} />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={chooseSourceFile}
                  aria-label={t("sshConfigImport.chooseSourceFile")}
                >
                  <FileUp size={16} />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => loadPreview(sourcePath)}
                  disabled={loadingPreview}
                >
                  {loadingPreview ? (
                    <Loader2 className="animate-spin" size={15} />
                  ) : (
                    <RefreshCcw size={15} />
                  )}
                  {t("sshConfigImport.preview")}
                </Button>
              </div>
            </div>

            <div>
              <Label htmlFor="ssh-config-group" className="mb-1.5 block">
                {t("sshConfigImport.group")}
              </Label>
              <Input
                id="ssh-config-group"
                value={group}
                onChange={(event) => setGroup(event.target.value)}
                placeholder={defaultGroup}
              />
            </div>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border px-3 py-2">
            <label className="flex cursor-pointer items-center gap-2 text-sm">
              <Checkbox
                checked={readyCount > 0 && selectedCount === readyCount}
                disabled={readyCount === 0}
                onCheckedChange={toggleAll}
                aria-label={t("sshConfigImport.selectAll")}
              />
              <span>
                {t("sshConfigImport.selectedCount", { selected: selectedCount, total: readyCount })}
              </span>
            </label>
            <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
              <label className="flex cursor-pointer items-center gap-2 text-sm">
                <Checkbox
                  checked={importForwards && forwardCount > 0}
                  disabled={forwardCount === 0}
                  onCheckedChange={setImportForwards}
                />
                <span>
                  {t("sshConfigImport.importForwards", {
                    count: forwardCount,
                    defaultValue: "Also import port forwards ({{count}})",
                  })}
                </span>
              </label>
              <label className="flex cursor-pointer items-center gap-2 text-sm">
                <Checkbox checked={overwriteExisting} onCheckedChange={setOverwriteExisting} />
                <span>{t("sshConfigImport.overwriteExisting")}</span>
              </label>
            </div>
          </div>

          {error && (
            <div className="border-destructive/30 bg-destructive/10 text-destructive rounded-lg border px-3 py-2 text-sm">
              {error}
            </div>
          )}

          {preview?.warnings.length ? (
            <div className="border-warning/30 bg-warning/10 text-warning-foreground rounded-lg border px-3 py-2 text-sm">
              {preview.warnings.join(" ")}
            </div>
          ) : null}

          <ScrollArea className="h-[360px] rounded-lg border">
            <div className="divide-border divide-y">
              {loadingPreview && (
                <div className="text-muted-foreground flex h-40 items-center justify-center gap-2 text-sm">
                  <Loader2 className="animate-spin" size={16} />
                  {t("sshConfigImport.loading")}
                </div>
              )}

              {!loadingPreview && preview && preview.hosts.length === 0 && (
                <div className="flex h-40 items-center justify-center px-6 text-center text-sm">
                  {t("sshConfigImport.empty")}
                </div>
              )}

              {!loadingPreview &&
                preview?.hosts.map((host) => (
                  <SshConfigImportHostRow
                    key={host.hostPattern}
                    host={host}
                    checked={selectedHosts.has(host.hostPattern)}
                    onCheckedChange={(next) => toggleHost(host.hostPattern, next)}
                  />
                ))}
            </div>
          </ScrollArea>
        </div>

        <DialogFooter className="border-border/80 border-t px-6 py-4">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button type="button" onClick={handleImport} disabled={importing || selectedCount === 0}>
            {importing ? <Loader2 className="animate-spin" size={15} /> : <Import size={15} />}
            {t("sshConfigImport.importSelected")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
