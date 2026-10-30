import { Check, Circle, Minus, X } from "lucide-react"
import React from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import type { ComposerRun, RunLineStatus } from "@/lib/composerRun"
import {
  continueComposerRun,
  dismissComposerRun,
  retryComposerRun,
  stopComposerRun,
} from "@/lib/composerRuns"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"

const STATUS_ICONS: Record<RunLineStatus, React.ReactNode> = {
  pending: <Circle className="text-muted-foreground size-3" />,
  running: <Spinner size={12} className="text-primary" />,
  succeeded: <Check className="size-3 text-emerald-500" />,
  failed: <X className="text-destructive size-3" />,
  unfinished: <Minus className="text-muted-foreground size-3" />,
}

/** What a line-by-line run has got to, with what the user can do about it. */
export const ComposerRunProgress: React.FC<{ tabId: string; run: ComposerRun }> = ({
  tabId,
  run,
}) => {
  const { t } = useTranslation()
  const total = run.lines.length
  const current = run.current + 1
  const failedCount = run.lines.filter((line) => line.status === "failed").length
  const summary = {
    running: t("composer.run.running", { current, total }),
    failed: t("composer.run.failed", { current, total, code: run.lines[run.current]?.exitCode }),
    done:
      failedCount > 0
        ? t("composer.run.doneWithFailures", { count: failedCount })
        : t("composer.run.done", { count: total }),
    stopped: t("composer.run.stopped", {
      count: run.lines.filter((line) => line.status === "pending").length,
    }),
  }[run.phase]

  const action = (label: string, onClick: () => void, variant: "outline" | "ghost" = "outline") => (
    <Button
      type="button"
      size="xs"
      variant={variant}
      onMouseDown={keepFocusOnMouseDown}
      onClick={onClick}
    >
      {label}
    </Button>
  )

  return (
    <div className="flex flex-col gap-1 text-xs" role="status">
      <div className="flex items-center gap-2">
        <span className={run.phase === "failed" ? "text-destructive" : "text-muted-foreground"}>
          {summary}
        </span>
        <div className="ml-auto flex items-center gap-1">
          {run.phase === "failed" && (
            <>
              {action(t("composer.run.retry"), () => retryComposerRun(tabId))}
              {action(t("composer.run.continue"), () => continueComposerRun(tabId))}
            </>
          )}
          {(run.phase === "running" || run.phase === "failed") &&
            action(t("composer.run.stop"), () => stopComposerRun(tabId))}
          {(run.phase === "done" || run.phase === "stopped") &&
            action(t("composer.run.dismiss"), () => dismissComposerRun(tabId), "ghost")}
        </div>
      </div>
      <ol className="max-h-28 overflow-auto font-mono">
        {run.lines.map((line, index) => (
          <li
            key={index}
            className={`flex items-center gap-1.5 ${line.status === "pending" ? "text-muted-foreground" : ""}`}
          >
            {STATUS_ICONS[line.status]}
            <span className="truncate whitespace-pre">{line.text}</span>
          </li>
        ))}
      </ol>
    </div>
  )
}
