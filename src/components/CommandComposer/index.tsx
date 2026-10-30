import "@/components/CommandComposer/CommandComposer.css"
import { Send, X } from "lucide-react"
import React, { useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"

import { useComposerHistory } from "@/components/CommandComposer/useComposerHistory"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { useConfig } from "@/contexts/ConfigContext"
import { useKeymap, useShortcutLabel } from "@/contexts/KeymapContext"
import {
  caretOnFirstLine,
  caretOnLastLine,
  closeComposer,
  composerInput,
  getComposerTarget,
  openComposer,
  useComposerState,
} from "@/lib/commandComposer"
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
  recentCommands: RecentCommand[]
}

/**
 * The compose bar below the panes: a command is edited here, where typing
 * waits on nothing, and goes to the focused terminal in one go on Enter.
 */
export const CommandComposer: React.FC<CommandComposerProps> = ({
  tabId,
  targetTitle,
  canSend,
  recentCommands,
}) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { registerHandler } = useKeymap()
  const shortcutLabel = useShortcutLabel()
  const { open, focusRequest } = useComposerState()
  const { text, setText, recallOlder, recallNewer } = useComposerHistory(recentCommands)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // The shortcut opens the bar, or brings the keyboard to it, and closes it
  // only from inside it, so it never closes a bar the user meant to reach.
  useEffect(
    () =>
      registerHandler("terminal.toggleComposer", () => {
        if (textareaRef.current && document.activeElement === textareaRef.current) {
          closeComposer()
          focusActiveTerminal()
        } else {
          openComposer()
        }
      }),
    [registerHandler]
  )

  useEffect(() => {
    if (focusRequest > 0) textareaRef.current?.focus()
  }, [focusRequest])

  if (!open || !tabId) return null

  /** Sends `data` to the focused terminal; false when it can't take it now. */
  const sendInput = (data: string) => {
    const target = canSend ? getComposerTarget(tabId) : undefined
    target?.input(data)
    return target !== undefined
  }

  const submit = () => {
    const target = canSend ? getComposerTarget(tabId) : undefined
    if (!target) return
    target.input(composerInput(text, target.bracketedPasteMode()))
    setText("")
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
    <div className="command-composer flex items-end gap-2 px-3 py-1.5">
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
        onChange={(event) => setText(event.target.value)}
        onKeyDown={handleKeyDown}
      />
      <Button
        type="button"
        size="xs"
        className="mb-0.5"
        disabled={!canSend}
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
  )
}
