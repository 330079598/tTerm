import { useMemo, useRef } from "react"
import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon } from "@xterm/addon-search"
import { type IDisposable, Terminal } from "@xterm/xterm"

import type { AppConfig } from "@/lib/appConfig"
import type { CommandMarks } from "@/components/TerminalTab/commandMarks"
import type { SavedPasswordPromptActions, TerminalTabProps } from "@/components/TerminalTab/types"
import { playBellSound } from "@/lib/bellSound"
import { markTabAttention } from "@/lib/tabAttention"
import type { TerminalAttentionEvent } from "@/lib/terminalNotifications"
import type { TerminalRenderer } from "@/lib/appConfig"
import { useStableRef } from "@/hooks/useStableRef"
import { compilePromptPatterns } from "@/lib/sudoPrompt"

type UseTerminalTabRefsOptions = Pick<
  TerminalTabProps,
  | "tabId"
  | "isActive"
  | "isGlobalShortcutTarget"
  | "connection"
  | "onPidChange"
  | "onCwdChange"
  | "onInput"
  | "onCommandExecuted"
  | "onReconnectRequest"
  | "onSavedPasswordPromptChange"
  | "onSessionUnavailable"
  | "onSensitivePrompt"
  | "onConnectionProgress"
  | "onAttention"
> & { config: AppConfig }

/**
 * The terminal tab's refs: its elements and terminal, and the latest props
 * and settings for callbacks that outlive a render.
 */
export function useTerminalTabRefs({
  tabId,
  isActive,
  isGlobalShortcutTarget,
  connection,
  onPidChange,
  onCwdChange,
  onInput,
  onCommandExecuted,
  onReconnectRequest,
  onSavedPasswordPromptChange,
  onSessionUnavailable,
  onSensitivePrompt,
  onConnectionProgress,
  onAttention,
  config,
}: UseTerminalTabRefsOptions) {
  const containerRef = useRef<HTMLDivElement>(null)
  const surfaceRef = useRef<HTMLDivElement>(null)
  const shellRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const searchAddonRef = useRef<SearchAddon | null>(null)
  const searchResultsDisposableRef = useRef<IDisposable | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const resizeRafRef = useRef<number | null>(null)
  const resizePtySyncTimerRef = useRef<number | null>(null)
  const activateFitTimerRef = useRef<number | null>(null)
  const lastPtySizeRef = useRef<{ rows: number; cols: number } | null>(null)
  const connectionRef = useStableRef(connection)
  const isActiveRef = useStableRef(isActive)
  // Shortcuts serve the focused pane only: every pane of a split is active.
  const isShortcutTargetRef = useStableRef(isGlobalShortcutTarget)
  const initializedRef = useRef(false)
  const creatingPtyRef = useRef(false)
  const waitingForReconnectRef = useRef(false)
  const onPidChangeRef = useStableRef(onPidChange)
  const onCwdChangeRef = useStableRef(onCwdChange)
  const onInputRef = useStableRef(onInput)
  const onCommandExecutedRef = useStableRef(onCommandExecuted)
  const onReconnectRequestRef = useStableRef(onReconnectRequest)
  const onSavedPasswordPromptChangeRef = useStableRef(onSavedPasswordPromptChange)
  const onSessionUnavailableRef = useStableRef(onSessionUnavailable)
  const onSensitivePromptRef = useStableRef(onSensitivePrompt)
  const onConnectionProgressRef = useStableRef(onConnectionProgress)
  const bellStyleRef = useStableRef(config.bell_style)
  // Output of a tab never looked at (restored in the background) is not news.
  const hasBeenActiveRef = useRef(isActive)
  const bellFlashTimerRef = useRef<number | null>(null)
  const onAttentionRef = useStableRef((event: TerminalAttentionEvent) => {
    if (event.kind === "bell") {
      const bellStyle = bellStyleRef.current
      if (bellStyle === "none") return
      if (bellStyle === "sound") {
        playBellSound()
      } else if (isActiveRef.current) {
        const surface = surfaceRef.current
        if (surface) {
          surface.classList.remove("terminal-bell-flash")
          // Restarts the animation for a bell during the previous flash.
          void surface.offsetWidth
          surface.classList.add("terminal-bell-flash")
          if (bellFlashTimerRef.current !== null) window.clearTimeout(bellFlashTimerRef.current)
          bellFlashTimerRef.current = window.setTimeout(() => {
            bellFlashTimerRef.current = null
            surface.classList.remove("terminal-bell-flash")
          }, 200)
        }
      }
    }
    onAttention?.(tabId, event)
  })
  const onBackgroundOutputRef = useStableRef(() => {
    if (hasBeenActiveRef.current) markTabAttention(tabId, "activity")
  })
  const configFontFamilyRef = useStableRef(config.font_family)
  const configFontSizeRef = useStableRef(config.font_size)
  const configLineHeightRef = useStableRef(config.terminal_line_height)
  const configLetterSpacingRef = useStableRef(config.terminal_letter_spacing)
  const configCursorStyleRef = useStableRef(config.cursor_style)
  const configMacOptionIsMetaRef = useStableRef(config.mac_option_is_meta)
  const configScrollbackLinesRef = useStableRef(config.scrollback_lines)
  const configTerminalRendererRef = useStableRef<TerminalRenderer>(config.terminal_renderer)
  const savedPasswordPromptActionsRef = useRef<SavedPasswordPromptActions | null>(null)
  const commandMarksRef = useRef<CommandMarks | null>(null)
  const commandMarksEnabledRef = useStableRef(config.command_marks)
  const sudoPromptPatterns = useMemo(
    () => compilePromptPatterns(config.sudo_prompt_patterns),
    [config.sudo_prompt_patterns]
  )
  const sudoPromptPatternsRef = useStableRef(sudoPromptPatterns)

  return {
    containerRef,
    surfaceRef,
    shellRef,
    termRef,
    fitAddonRef,
    searchAddonRef,
    searchResultsDisposableRef,
    resizeObserverRef,
    resizeRafRef,
    resizePtySyncTimerRef,
    activateFitTimerRef,
    lastPtySizeRef,
    connectionRef,
    isActiveRef,
    isShortcutTargetRef,
    initializedRef,
    creatingPtyRef,
    waitingForReconnectRef,
    onPidChangeRef,
    onCwdChangeRef,
    onInputRef,
    onCommandExecutedRef,
    onReconnectRequestRef,
    onSavedPasswordPromptChangeRef,
    onSessionUnavailableRef,
    onSensitivePromptRef,
    onConnectionProgressRef,
    bellStyleRef,
    hasBeenActiveRef,
    bellFlashTimerRef,
    onAttentionRef,
    onBackgroundOutputRef,
    configFontFamilyRef,
    configFontSizeRef,
    configLineHeightRef,
    configLetterSpacingRef,
    configCursorStyleRef,
    configMacOptionIsMetaRef,
    configScrollbackLinesRef,
    configTerminalRendererRef,
    savedPasswordPromptActionsRef,
    commandMarksRef,
    commandMarksEnabledRef,
    sudoPromptPatternsRef,
  }
}

export type TerminalTabRefs = ReturnType<typeof useTerminalTabRefs>
