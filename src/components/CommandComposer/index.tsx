import "@/components/CommandComposer/CommandComposer.css"
import { Send, X } from "lucide-react"
import React, { useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { ComposerRunProgress } from "@/components/CommandComposer/ComposerRunProgress"
import { useComposerHistory } from "@/components/CommandComposer/useComposerHistory"
import { useComposerShortcut } from "@/components/CommandComposer/useComposerShortcut"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { useConfig } from "@/contexts/ConfigContext"
import { useShortcutLabel } from "@/contexts/KeymapContext"
import {
  caretOnFirstLine,
  caretOnLastLine,
  closeComposer,
  composerInput,
  getComposerTarget,
  setComposerLineByLine,
  useComposerState,
} from "@/lib/commandComposer"
import { isRunActive, runLines } from "@/lib/composerRun"
import { dismissComposerRun, startComposerRun, useComposerRun } from "@/lib/composerRuns"
import { isImeKeyEvent } from "@/lib/ime"
import { withShortcut } from "@/lib/keymap/keymap"
import { focusActiveTerminal, keepFocusOnMouseDown } from "@/lib/terminalFocus"
import type { RecentCommand } from "@/types/command"

/** The most lines the bar grows to before it scrolls. */
const MAX_ROWS = 6

interface CommandComposerProps {
  /** The focused pane's terminal, or null when no terminal the bar can send to is in sight. */
  tabId: string | null
  targetTitle: string
  /** The terminal is connected, so what is sent reaches it. */
  canSend: boolean
  /** An SSH session, whose shell integration is set per connection. */
  isRemote: boolean
  recentCommands: RecentCommand[]
}

/**
 * The compose bar below the panes: a command is edited here, where typing
 * waits on nothing, and goes to the focused terminal in one go on Enter.
 * Several lines go in as one paste, or run one at a time.
 */
export const CommandComposer: React.FC<CommandComposerProps> = ({
  tabId,
  targetTitle,
  canSend,
  isRemote,
  recentCommands,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const shortcutLabel = useShortcutLabel()
  const { open, focusRequest, lineByLine } = useComposerState()
  const { text, setText, recallOlder, recallNewer } = useComposerHistory(recentCommands)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  useComposerShortcut(textareaRef, focusRequest)
  const run = useComposerRun(tabId)
  // Why the last Enter sent nothing, for the tab it was pressed in.
  const [notice, setNotice] = useState<{ tabId: string; message: string } | null>(null)

  if (!open || !tabId) return null
  const runActive = isRunActive(run)

  /** Sends `data` to the focused terminal; false when it can't take it now. */
  const sendInput = (data: string) => {
    const target = canSend ? getComposerTarget(tabId) : undefined
    target?.input(data)
    return target !== undefined
  }

  const submit = () => {
    const target = canSend ? getComposerTarget(tabId) : undefined
    if (!target || runActive) return
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
    setText("")
    setNotice(null)
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (isImeKeyEvent(event.nativeEvent)) return
    const { selectionStart, selectionEnd } = event.currentTarget
    const noSelection = selectionStart === selectionEnd
    const plainCtrl = event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey
    if (event.key === "Enter" && !event.shiftKey && !event.altKey) {
      event.preventDefault()
      submit()
    } else if (event.key === "Escape") {
      event.preventDefault()
      focusActiveTerminal()
    } else if (event.key === "ArrowUp" && noSelection && caretOnFirstLine(text, selectionStart)) {
      if (recallOlder()) event.preventDefault()
    } else if (event.key === "ArrowDown" && noSelection && caretOnLastLine(text, selectionEnd)) {
      if (recallNewer()) event.preventDefault()
    } else if (plainCtrl && event.key.toLowerCase() === "c" && noSelection) {
      // Interrupts what runs without leaving the bar; with text selected,
      // Ctrl+C copies it as usual.
      if (sendInput("\x03")) event.preventDefault()
    } else if (plainCtrl && event.key.toLowerCase() === "d" && !text) {
      if (sendInput("\x04")) event.preventDefault()
    }
  }

  const rows = Math.min(MAX_ROWS, text.split("\n").length)

  return (
    <div className="command-composer flex flex-col gap-1 px-3 py-1.5">
      {run && <ComposerRunProgress tabId={tabId} run={run} />}
      {notice?.tabId === tabId && (
        <p className="text-destructive text-xs" role="alert">
          {notice.message}
        </p>
      )}
      <div className="flex items-end gap-2">
        <Textarea
          ref={textareaRef}
          aria-label={t("composer.title")}
          title={t("composer.keysHint")}
          className="min-h-0 flex-1 resize-none px-2 py-1 text-xs md:text-xs"
          style={{ fontFamily: config.font_family }}
          rows={rows}
          value={text}
          placeholder={
            canSend
              ? t("composer.placeholder", { name: targetTitle })
              : t("composer.unavailable", { name: targetTitle })
          }
          onChange={(event) => {
            setText(event.target.value)
            setNotice(null)
          }}
          onKeyDown={handleKeyDown}
        />
        <div className="mb-1 flex items-center gap-1.5" title={t("composer.lineByLineHint")}>
          <Checkbox
            id="command-composer-line-by-line"
            checked={lineByLine}
            onMouseDown={keepFocusOnMouseDown}
            onCheckedChange={setComposerLineByLine}
          />
          <Label htmlFor="command-composer-line-by-line" className="text-xs font-normal">
            {t("composer.lineByLine")}
          </Label>
        </div>
        <Button
          type="button"
          size="xs"
          className="mb-0.5"
          disabled={!canSend || runActive}
          onMouseDown={keepFocusOnMouseDown}
          onClick={submit}
        >
          <Send />
          {t("composer.send")}
        </Button>
        <Button
          type="button"
          size="icon-xs"
          variant="ghost"
          className="mb-0.5"
          aria-label={t("composer.close")}
          title={withShortcut(t("composer.close"), shortcutLabel("terminal.toggleComposer"))}
          onMouseDown={keepFocusOnMouseDown}
          onClick={() => {
            closeComposer()
            focusActiveTerminal()
          }}
        >
          <X />
        </Button>
      </div>
    </div>
  )
}
