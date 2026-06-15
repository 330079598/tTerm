import React, { useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import {
  AlertTriangle,
  CheckCircle2,
  FileSearch,
  FileUp,
  FolderOpen,
  Import,
  Loader2,
  RefreshCcw,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
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
import { cn } from "@/lib/utils"
import type { SavedJumpHost, SavedProfile } from "@/types/tab"

interface SshConfigImportDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onImported: (profiles: SavedProfile[]) => void
}

interface SshConfigImportHost {
  hostPattern: string
  name: string
  host?: string
  port: number
  username?: string
  authMethod: string
  privateKeyPath?: string
  keepaliveIntervalSecs: number
  keepaliveCountMax: number
  jumpHosts: SavedJumpHost[]
  warnings: string[]
  unsupportedOptions: string[]
  skipped: boolean
  skipReason?: string
  existingProfileId?: string
}

interface SshConfigImportPreview {
  sourcePath: string
  hosts: SshConfigImportHost[]
  warnings: string[]
}

interface SshConfigImportResult {
  imported: number
  updated: number
  skipped: number
  profiles: SavedProfile[]
}

const DEFAULT_GROUP = "Imported from SSH config"

const statusForHost = (host: SshConfigImportHost) => {
  if (host.skipped) return "skipped"
  if (host.unsupportedOptions.length > 0 || host.warnings.length > 0) return "warning"
  return "ready"
}

export const SshConfigImportDialog: React.FC<SshConfigImportDialogProps> = ({
  open,
  onOpenChange,
  onImported,
}) => {
  const { t } = useTranslation()
  const [sourcePath, setSourcePath] = useState("~/.ssh")
  const [group, setGroup] = useState(DEFAULT_GROUP)
  const [overwriteExisting, setOverwriteExisting] = useState(false)
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

  const loadPreview = async (path: string) => {
    setLoadingPreview(true)
    setError(null)

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

  useEffect(() => {
    if (!open) {
      return
    }

    void loadPreview(sourcePath)
    // Run only when the dialog opens; manual path edits are previewed by the Preview button.
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
        },
      })
      onImported(result.profiles)
      toast({
        title: t("sshConfigImport.importComplete"),
        description: t("sshConfigImport.importCompleteDesc", {
          imported: result.imported,
          updated: result.updated,
          skipped: result.skipped,
        }),
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
                placeholder={DEFAULT_GROUP}
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
            <label className="flex cursor-pointer items-center gap-2 text-sm">
              <Checkbox checked={overwriteExisting} onCheckedChange={setOverwriteExisting} />
              <span>{t("sshConfigImport.overwriteExisting")}</span>
            </label>
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
                preview?.hosts.map((host) => {
                  const status = statusForHost(host)
                  const checked = selectedHosts.has(host.hostPattern)
                  const details = [
                    host.username ? `${host.username}@${host.host ?? host.name}` : host.host,
                    host.port !== 22 ? `:${host.port}` : null,
                    host.authMethod === "key"
                      ? t("profiles.authMethodKey")
                      : t("profiles.authMethodPassword"),
                    host.jumpHosts.length > 0
                      ? t("jumpHost.viaCount", { count: host.jumpHosts.length })
                      : null,
                  ].filter(Boolean)

                  return (
                    <div
                      key={host.hostPattern}
                      className={cn(
                        "grid gap-3 px-4 py-3 transition-colors",
                        !host.skipped && "hover:bg-muted/25"
                      )}
                    >
                      <div className="flex items-start gap-3">
                        <Checkbox
                          className="mt-1"
                          checked={checked}
                          disabled={host.skipped}
                          onCheckedChange={(next) => toggleHost(host.hostPattern, next)}
                          aria-label={t("sshConfigImport.selectHost", { name: host.name })}
                        />
                        <div className="min-w-0 flex-1">
                          <div className="flex min-w-0 flex-wrap items-center gap-2">
                            <span className="truncate text-sm font-medium">{host.name}</span>
                            {host.existingProfileId && (
                              <Badge variant="secondary">{t("sshConfigImport.exists")}</Badge>
                            )}
                            {status === "ready" && (
                              <Badge variant="outline" className="text-success">
                                <CheckCircle2 size={12} />
                                {t("sshConfigImport.ready")}
                              </Badge>
                            )}
                            {status === "warning" && (
                              <Badge variant="outline" className="text-warning">
                                <AlertTriangle size={12} />
                                {t("sshConfigImport.warning")}
                              </Badge>
                            )}
                            {status === "skipped" && (
                              <Badge variant="outline" className="text-muted-foreground">
                                {t("sshConfigImport.skipped")}
                              </Badge>
                            )}
                          </div>
                          <div className="text-muted-foreground mt-1 truncate text-xs">
                            {details.length > 0
                              ? details.join(" ")
                              : t("profiles.connectionDetailsUnavailable")}
                          </div>
                        </div>
                      </div>

                      {(host.skipReason ||
                        host.warnings.length > 0 ||
                        host.unsupportedOptions.length > 0) && (
                        <div className="text-muted-foreground ml-7 grid gap-1 text-xs">
                          {host.skipReason && <div>{host.skipReason}</div>}
                          {host.warnings.map((warning) => (
                            <div key={warning}>{warning}</div>
                          ))}
                          {host.unsupportedOptions.length > 0 && (
                            <div>
                              {t("sshConfigImport.unsupportedOptions", {
                                options: host.unsupportedOptions.join(", "),
                              })}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}
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
