import React, { useCallback, useId, useRef, useState } from "react"
import { AlertTriangle } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { formatBytes, formatTimestamp } from "@/components/SftpDrawer/sftpDrawerUtils"
import type {
  ConflictPolicy,
  ConflictReport,
  PromptConflictPolicy,
  TransferConflict,
} from "@/components/SftpDrawer/types"

type Direction = "upload" | "download"

interface PendingPrompt {
  direction: Direction
  report: ConflictReport
}

const POLICY_ORDER: ConflictPolicy[] = ["overwriteIfNewer", "overwrite", "skip", "rename"]

function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "")
  const index = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"))
  return index >= 0 ? trimmed.slice(index + 1) : trimmed
}

function formatMtime(seconds?: number | null): string {
  return seconds == null ? "--" : formatTimestamp(seconds * 1000)
}

/** Which side carries the strictly newer mtime, if that can be told. */
function newerSide(conflict: TransferConflict): "source" | "target" | null {
  const { sourceMtime, targetMtime } = conflict
  if (sourceMtime == null || targetMtime == null || sourceMtime === targetMtime) return null
  return sourceMtime > targetMtime ? "source" : "target"
}

interface ConflictRowProps {
  conflict: TransferConflict
  direction: Direction
}

const ConflictRow: React.FC<ConflictRowProps> = ({ conflict, direction }) => {
  const { t } = useTranslation()
  const newer = newerSide(conflict)
  const sourceLabel =
    direction === "upload"
      ? t("sftp.conflicts.local", { defaultValue: "Local" })
      : t("sftp.conflicts.remote", { defaultValue: "Remote" })
  const targetLabel =
    direction === "upload"
      ? t("sftp.conflicts.remote", { defaultValue: "Remote" })
      : t("sftp.conflicts.local", { defaultValue: "Local" })
  const newerBadge = (
    <span className="text-primary font-medium">
      {t("sftp.conflicts.newer", { defaultValue: "newer" })}
    </span>
  )

  return (
    <li className="border-border/60 border-b px-3 py-2 last:border-b-0">
      <div className="truncate text-xs font-medium" title={conflict.targetPath}>
        {baseName(conflict.targetPath)}
      </div>
      <div className="text-muted-foreground truncate text-[10px]" title={conflict.targetPath}>
        {conflict.targetPath}
      </div>
      <div className="text-muted-foreground mt-1 grid grid-cols-[auto_1fr] gap-x-2 text-[10px]">
        <span>{sourceLabel}</span>
        <span className="flex gap-2">
          <span>{formatBytes(conflict.sourceSize)}</span>
          <span>{formatMtime(conflict.sourceMtime)}</span>
          {newer === "source" && newerBadge}
        </span>
        <span>{targetLabel}</span>
        {conflict.targetIsDir ? (
          <span className="text-destructive">
            {t("sftp.conflicts.targetIsFolder", { defaultValue: "A folder with this name" })}
          </span>
        ) : (
          <span className="flex gap-2">
            <span>{formatBytes(conflict.targetSize)}</span>
            <span>{formatMtime(conflict.targetMtime)}</span>
            {newer === "target" && newerBadge}
          </span>
        )}
      </div>
    </li>
  )
}

/**
 * Promise-based prompt for resolving transfer name conflicts. The last choice
 * becomes the preselected option next time, so repeated uploads of the same
 * kind need a single Enter.
 */
export function useSftpConflictDialog() {
  const { t } = useTranslation()
  const [pending, setPending] = useState<PendingPrompt | null>(null)
  const [policy, setPolicy] = useState<ConflictPolicy>("overwriteIfNewer")
  const lastPolicyRef = useRef<ConflictPolicy>("overwriteIfNewer")
  const groupName = useId()

  const resolveRef = useRef<((policy: ConflictPolicy | null) => void) | null>(null)

  const promptConflictPolicy = useCallback<PromptConflictPolicy>((report, direction) => {
    return new Promise((resolve) => {
      // A newer prompt supersedes one still open: its transfer is cancelled
      // rather than left waiting on a dialog that is no longer shown.
      resolveRef.current?.(null)
      resolveRef.current = resolve
      setPolicy(lastPolicyRef.current)
      setPending({ direction, report })
    })
  }, [])

  const close = useCallback((chosen: ConflictPolicy | null) => {
    if (chosen) lastPolicyRef.current = chosen
    resolveRef.current?.(chosen)
    resolveRef.current = null
    setPending(null)
  }, [])

  const policyLabels: Record<ConflictPolicy, { title: string; description: string }> = {
    overwriteIfNewer: {
      title: t("sftp.conflicts.policies.overwriteIfNewer", {
        defaultValue: "Overwrite only if newer",
      }),
      description: t("sftp.conflicts.policies.overwriteIfNewerDesc", {
        defaultValue:
          "Replace a file only when the incoming one was modified later; skip the rest.",
      }),
    },
    overwrite: {
      title: t("sftp.conflicts.policies.overwrite", { defaultValue: "Overwrite all" }),
      description: t("sftp.conflicts.policies.overwriteDesc", {
        defaultValue: "Replace every existing file with the incoming one.",
      }),
    },
    skip: {
      title: t("sftp.conflicts.policies.skip", { defaultValue: "Skip all" }),
      description: t("sftp.conflicts.policies.skipDesc", {
        defaultValue: "Keep existing files and transfer only the rest.",
      }),
    },
    rename: {
      title: t("sftp.conflicts.policies.rename", { defaultValue: "Keep both" }),
      description: t("sftp.conflicts.policies.renameDesc", {
        defaultValue: 'Save incoming files under a new name such as "name (1).ext".',
      }),
    },
  }

  // Rendered as an element rather than a component: a component identity that
  // changes with every render would remount the dialog and drop focus.
  let conflictDialog: React.ReactNode = null
  if (pending) {
    const { direction, report } = pending
    const hidden = report.total - report.conflicts.length

    conflictDialog = (
      <Dialog open onOpenChange={(open) => !open && close(null)}>
        <DialogContent
          className="sm:max-w-lg"
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent.isComposing) {
              event.preventDefault()
              close(policy)
            }
          }}
        >
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <AlertTriangle className="size-4 text-amber-500" />
              {t("sftp.conflicts.title", {
                count: report.total,
                defaultValue: `${report.total} file(s) already exist`,
              })}
            </DialogTitle>
            <DialogDescription>
              {direction === "upload"
                ? t("sftp.conflicts.uploadDescription", {
                    count: report.total,
                    fileCount: report.fileCount,
                    defaultValue: `${report.total} of ${report.fileCount} file(s) to upload already exist on the server.`,
                  })
                : t("sftp.conflicts.downloadDescription", {
                    count: report.total,
                    fileCount: report.fileCount,
                    defaultValue: `${report.total} of ${report.fileCount} file(s) to download already exist locally.`,
                  })}
            </DialogDescription>
          </DialogHeader>

          <ul className="border-border max-h-48 overflow-y-auto rounded-md border">
            {report.conflicts.map((conflict) => (
              <ConflictRow key={conflict.targetPath} conflict={conflict} direction={direction} />
            ))}
            {hidden > 0 && (
              <li className="text-muted-foreground px-3 py-2 text-[10px]">
                {t("sftp.conflicts.more", {
                  count: hidden,
                  defaultValue: `and ${hidden} more`,
                })}
              </li>
            )}
          </ul>

          <div
            role="radiogroup"
            aria-label={t("sftp.conflicts.policyLabel", { defaultValue: "For existing files" })}
            className="grid gap-2"
          >
            {POLICY_ORDER.map((option) => (
              <label
                key={option}
                className={`flex cursor-pointer items-start gap-2 rounded-md border px-3 py-2 transition-colors ${
                  policy === option
                    ? "border-primary bg-primary/5"
                    : "border-border hover:bg-muted/50"
                }`}
              >
                <input
                  type="radio"
                  name={groupName}
                  value={option}
                  checked={policy === option}
                  onChange={() => setPolicy(option)}
                  className="accent-primary mt-0.5"
                />
                <span className="grid gap-0.5">
                  <span className="text-sm">{policyLabels[option].title}</span>
                  <span className="text-muted-foreground text-xs">
                    {policyLabels[option].description}
                  </span>
                </span>
              </label>
            ))}
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => close(null)}>
              {t("sftp.conflicts.cancel", { defaultValue: "Cancel transfer" })}
            </Button>
            <Button type="button" data-dialog-initial-focus="true" onClick={() => close(policy)}>
              {t("sftp.conflicts.continue", { defaultValue: "Continue" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    )
  }

  return { conflictDialog, promptConflictPolicy }
}
