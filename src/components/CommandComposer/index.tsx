import "@/components/CommandComposer/CommandComposer.css"
import { Send, X } from "lucide-react"
import React, { useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { ComposerRunProgress } from "@/components/CommandComposer/ComposerRunProgress"
import { useComposerHistory } from "@/components/CommandComposer/useComposerHistory"
import { useComposerShortcut } from "@/components/CommandComposer/useComposerShortcut"
import {
  useComposerSubmit,
  type ComposerBroadcast,
} from "@/components/CommandComposer/useComposerSubmit"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import { useConfig } from "@/contexts/ConfigContext"
import { useShortcutLabel } from "@/contexts/KeymapContext"
import {
  caretOnFirstLine,
  caretOnLastLine,
  closeComposer,
  getComposerTarget,
  historySuggestion,
  setComposerLineByLine,
  useComposerState,
} from "@/lib/commandComposer"
import { isRunActive } from "@/lib/composerRun"
import { useComposerRun } from "@/lib/composerRuns"
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
  broadcast: ComposerBroadcast
}

/**
 * The compose bar below the panes: a command is edited here, where typing
 * waits on nothing, and goes to the focused terminal, or to the broadcast
 * targets, in one go on Enter. Several lines go in as one paste, or run one
 * at a time.
 */
export const CommandComposer: React.FC<CommandComposerProps> = ({
  tabId,
  targetTitle,
  canSend,
  isRemote,
  recentCommands,
  broadcast,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const shortcutLabel = useShortcutLabel()
  const { open, focusRequest, lineByLine, text } = useComposerState()
  const { history, setText, recallOlder, recallNewer } = useComposerHistory(text, recentCommands)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  useComposerShortcut(textareaRef, focusRequest)
  const run = useComposerRun(tabId)
  const [broadcastPicked, setBroadcastPicked] = useState(false)
  // The choice lasts only while the broadcast panel has targets picked.
  const toBroadcast = broadcastPicked && broadcast.targetCount > 0
  const { submit, sending, notice, clearNotice } = useComposerSubmit({
    tabId: tabId ?? "",
    canSend,
    isRemote,
    lineByLine,
    toBroadcast,
    broadcast,
    text,
    setText,
  })

  if (!open || !tabId) return null
  const runActive = isRunActive(run)
  const canSubmit = toBroadcast ? !sending : canSend && !runActive
  const suggestion = historySuggestion(text, history)

  /** Sends `data` to the focused terminal; false when it can't take it now. */
  const sendInput = (data: string) => {
    const target = canSend ? getComposerTarget(tabId) : undefined
    target?.input(data)
    return target !== undefined
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (isImeKeyEvent(event.nativeEvent)) return
    const { selectionStart, selectionEnd } = event.currentTarget
    const noSelection = selectionStart === selectionEnd
    const plainCtrl = event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey
    if (event.key === "Enter" && !event.shiftKey && !event.altKey) {
      event.preventDefault()
      if (canSubmit) submit()
    } else if (event.key === "Escape") {
      event.preventDefault()
      focusActiveTerminal()
    } else if (
      (event.key === "ArrowRight" || event.key === "End") &&
      suggestion &&
      noSelection &&
      selectionEnd === text.length
    ) {
      event.preventDefault()
      setText(suggestion)
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
  const fontStyle = { fontFamily: config.font_family }
  let placeholder = t("composer.placeholder", { name: targetTitle })
  if (toBroadcast)
    placeholder = t("composer.placeholderBroadcast", { count: broadcast.targetCount })
  else if (!canSend) placeholder = t("composer.unavailable", { name: targetTitle })

  return (
    <div className="command-composer flex flex-col gap-1 px-3 py-1.5">
      {run && <ComposerRunProgress tabId={tabId} run={run} />}
      {notice && (
        <p className="text-destructive text-xs" role="alert">
          {notice}
        </p>
      )}
      <div className="flex items-end gap-2">
        <div className="relative min-w-0 flex-1">
          <Textarea
            ref={textareaRef}
            aria-label={t("composer.title")}
            title={t("composer.keysHint")}
            className="min-h-0 resize-none px-2 py-1 text-xs md:text-xs"
            style={fontStyle}
            rows={rows}
            value={text}
            placeholder={placeholder}
            onChange={(event) => {
              setText(event.target.value)
              clearNotice()
            }}
            onKeyDown={handleKeyDown}
          />
          {suggestion && (
            // Drawn over the text in the same font, padding and border, so the
            // rest of the suggestion follows on where the typed text ends.
            <div
              aria-hidden="true"
              className="pointer-events-none absolute inset-0 overflow-hidden rounded-md border border-transparent px-2 py-1 text-xs break-words whitespace-pre-wrap"
              style={fontStyle}
            >
              <span className="invisible">{text}</span>
              <span className="text-muted-foreground">{suggestion.slice(text.length)}</span>
            </div>
          )}
        </div>
        {broadcast.targetCount > 0 && (
          <Select
            aria-label={t("composer.target")}
            className="mb-0.5 h-6 w-auto text-xs"
            value={toBroadcast ? "broadcast" : "pane"}
            onChange={(event) => setBroadcastPicked(event.target.value === "broadcast")}
          >
            <option value="pane">{t("composer.targetPane")}</option>
            <option value="broadcast">
              {t("composer.targetBroadcast", { count: broadcast.targetCount })}
            </option>
          </Select>
        )}
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
          disabled={!canSubmit}
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
