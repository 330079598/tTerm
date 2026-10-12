import type { TFunction } from "i18next"
import { useCallback, useEffect } from "react"
import { Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { AppConfig } from "@/lib/appConfig"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { toast } from "@/hooks/use-toast"
import { useStableRef } from "@/hooks/useStableRef"
import { pasteNeedsConfirmation, summarizePaste } from "@/lib/pasteGuard"

type UseTerminalClipboardOptions = Pick<
  TerminalTabProps,
  "isBroadcastSource" | "liveBroadcastState"
> &
  Pick<TerminalTabRefs, "termRef" | "containerRef"> & { config: AppConfig; t: TFunction }

/**
 * Pasting into the terminal (asking first when a line break would run a
 * command) and copying on select.
 */
export function useTerminalClipboard({
  isBroadcastSource,
  liveBroadcastState,
  termRef,
  containerRef,
  config,
  t,
}: UseTerminalClipboardOptions) {
  const { confirm: confirmPaste, ConfirmDialog: PasteConfirmDialog } = useConfirmDialog()
  const confirmMultilinePasteRef = useStableRef(config.confirm_multiline_paste)
  // A live broadcast source asks on its own before a multiline paste goes out.
  const broadcastConfirmsPasteRef = useStableRef(
    isBroadcastSource && liveBroadcastState === "active"
  )

  const pasteNeedsConfirm = useCallback(
    (term: Terminal, text: string) =>
      !broadcastConfirmsPasteRef.current &&
      pasteNeedsConfirmation(text, {
        enabled: confirmMultilinePasteRef.current,
        bracketedPasteMode: term.modes.bracketedPasteMode,
      }),
    [broadcastConfirmsPasteRef, confirmMultilinePasteRef]
  )

  /** Pastes into the terminal, first asking when a line break would run a command. */
  const pasteIntoTerminal = useCallback(
    async (text: string) => {
      const term = termRef.current
      if (!term || !text) return
      if (pasteNeedsConfirm(term, text)) {
        const { lineCount, preview } = summarizePaste(text)
        const confirmed = await confirmPaste({
          title: t("terminalPaste.multilineTitle"),
          description: (
            <>
              {lineCount > 1
                ? t("terminalPaste.multilineDescription", { count: lineCount })
                : t("terminalPaste.trailingLineBreakDescription")}
              <span className="bg-muted text-foreground mt-3 block max-h-48 overflow-auto rounded-md border px-2 py-1.5 font-mono text-xs whitespace-pre">
                {preview}
              </span>
            </>
          ),
          confirmText: t("terminalPaste.pasteAnyway"),
          defaultAction: "confirm",
        })
        // The terminal may have been recreated while the dialog was open.
        if (termRef.current !== term) return
        term.focus()
        if (!confirmed) return
      }
      term.paste(text)
    },
    [confirmPaste, pasteNeedsConfirm, t, termRef]
  )

  // Keyboard and menu-bar pastes reach xterm as DOM paste events; hold back
  // the ones that need confirmation before xterm sends them.
  useEffect(() => {
    const handlePaste = (event: ClipboardEvent) => {
      const container = containerRef.current
      const term = termRef.current
      if (!container || !term) return
      if (!(event.target instanceof Node) || !container.contains(event.target)) return
      const text = event.clipboardData?.getData("text/plain") ?? ""
      if (!pasteNeedsConfirm(term, text)) return
      event.preventDefault()
      event.stopPropagation()
      void pasteIntoTerminal(text)
    }
    document.addEventListener("paste", handlePaste, true)
    return () => document.removeEventListener("paste", handlePaste, true)
  }, [containerRef, pasteIntoTerminal, pasteNeedsConfirm, termRef])

  const pasteFromClipboard = useCallback(async () => {
    try {
      const clipboardText = await invoke<string>("plugin:clipboard-manager|read_text")
      if (clipboardText) await pasteIntoTerminal(clipboardText)
    } catch (error) {
      console.error("Failed to paste into terminal:", error)
      toast({
        title: t("terminalContext.pasteFailedTitle"),
        description: t("terminalContext.pasteFailedDescription"),
        variant: "destructive",
      })
    }
  }, [pasteIntoTerminal, t])

  // Copy on select: the selection is final once the mouse button that made
  // it is released, which may happen outside the terminal.
  useEffect(() => {
    const container = containerRef.current
    if (!config.copy_on_select || !container) return

    let copyTimer: number | null = null
    const copySelection = () => {
      copyTimer = window.setTimeout(() => {
        copyTimer = null
        const term = termRef.current
        const selection = term?.hasSelection() ? term.getSelection() : ""
        if (!selection) return
        invoke("plugin:clipboard-manager|write_text", { text: selection }).catch((error) => {
          console.error("Failed to copy terminal selection:", error)
        })
      }, 0)
    }
    const handleMouseDown = (event: MouseEvent) => {
      if (event.button !== 0) return
      document.removeEventListener("mouseup", copySelection, true)
      document.addEventListener("mouseup", copySelection, { capture: true, once: true })
    }

    container.addEventListener("mousedown", handleMouseDown, true)
    return () => {
      container.removeEventListener("mousedown", handleMouseDown, true)
      document.removeEventListener("mouseup", copySelection, true)
      if (copyTimer !== null) window.clearTimeout(copyTimer)
    }
  }, [config.copy_on_select, containerRef, termRef])

  return {
    PasteConfirmDialog,
    pasteFromClipboard,
  }
}
