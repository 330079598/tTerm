import { useState } from "react"
import { useTranslation } from "react-i18next"

import { toast } from "@/hooks/use-toast"
import { composerInput, getComposerTarget } from "@/lib/commandComposer"
import { runLines } from "@/lib/composerRun"
import { dismissComposerRun, startComposerRun } from "@/lib/composerRuns"

/** The terminals picked in the broadcast panel, as the bar sends to them. */
export interface ComposerBroadcast {
  /** How many are picked, while no live broadcast runs; 0 hides the choice. */
  targetCount: number
  /** Sends one command to them all, as the panel does; false when it didn't go. */
  send: (command: string) => Promise<boolean>
  /** The picked terminals that are connected now. */
  connectedTargetIds: () => string[]
  /** Keeps a command sent to them among the recent ones, as typing it would. */
  record: (command: string) => void
}

interface UseComposerSubmitOptions {
  tabId: string
  canSend: boolean
  isRemote: boolean
  lineByLine: boolean
  /** Sends to the broadcast targets rather than the focused pane. */
  toBroadcast: boolean
  broadcast: ComposerBroadcast
  text: string
  setText: (text: string) => void
}

/**
 * What Enter does with the bar's text: one paste or a line-by-line run, in
 * the focused pane or in every broadcast target, and why when nothing went.
 */
export function useComposerSubmit({
  tabId,
  canSend,
  isRemote,
  lineByLine,
  toBroadcast,
  broadcast,
  text,
  setText,
}: UseComposerSubmitOptions) {
  const { t } = useTranslation()
  // Why the last Enter sent nothing, for the tab it was pressed in.
  const [notice, setNotice] = useState<{ tabId: string; message: string } | null>(null)
  const [sending, setSending] = useState(false)

  const done = () => {
    setText("")
    setNotice(null)
  }

  const submitToPane = () => {
    const target = canSend ? getComposerTarget(tabId) : undefined
    if (!target) return
    const lines = runLines(text)
    if (lineByLine && lines.length > 1) {
      const started = startComposerRun(tabId, lines)
      if (started !== "started") {
        const reason =
          started === "busy" ? "shellBusy" : isRemote ? "unmarkedRemote" : "unmarkedLocal"
        setNotice({ tabId, message: t(`composer.${reason}`) })
        return
      }
    } else {
      target.input(composerInput(text, target.bracketedPasteMode()))
      dismissComposerRun(tabId)
    }
    done()
  }

  const runInBroadcastTargets = (lines: string[]) => {
    const started = broadcast
      .connectedTargetIds()
      .filter((id) => startComposerRun(id, lines) === "started")
    if (started.length === 0) {
      setNotice({ tabId, message: t("composer.broadcastNoneStarted") })
      return
    }
    const skipped = broadcast.targetCount - started.length
    toast({
      title: t("composer.broadcastRunsStarted", { count: started.length }),
      description:
        skipped > 0
          ? t("composer.broadcastRunsSkipped", { count: skipped })
          : t("composer.broadcastRunsHint"),
      variant: skipped > 0 ? "destructive" : "default",
    })
    done()
  }

  const submitToBroadcast = async () => {
    const lines = runLines(text)
    if (lineByLine && lines.length > 1) {
      runInBroadcastTargets(lines)
      return
    }
    const command = text.replace(/[\r\n]+$/, "")
    setSending(true)
    try {
      if (!(await broadcast.send(command))) return
      // Written straight to each terminal, so their shells' lines don't note it.
      if (command.trim()) broadcast.record(command)
      done()
    } finally {
      setSending(false)
    }
  }

  const submit = () => {
    if (sending) return
    if (toBroadcast) void submitToBroadcast()
    else submitToPane()
  }

  return {
    submit,
    sending,
    notice: notice?.tabId === tabId ? notice.message : null,
    clearNotice: () => setNotice(null),
  }
}
