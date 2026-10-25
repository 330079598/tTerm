import type { TFunction } from "i18next"
import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon, type ISearchResultChangeEvent } from "@xterm/addon-search"
import { type IDisposable, Terminal } from "@xterm/xterm"

import type { TerminalCommandCaptureState } from "@/lib/terminalCommandCapture"
import type { LoginScriptRunner } from "@/lib/loginScript"
import type { createScrollbackMemory } from "@/components/TerminalTab/scrollbackMemory"
import type { TrackedTerminalContent } from "@/components/TerminalTab/savedTerminalContent"
import { CommandMarks } from "@/components/TerminalTab/commandMarks"
import type { KeywordHighlighter } from "@/components/TerminalTab/keywordHighlighter"
import type {
  ConnectionState,
  HostKeyPromptState,
  SavedPasswordPromptActions,
  SavedPasswordPromptState,
  SshConnectionProgress,
  TerminalTabProps,
} from "@/components/TerminalTab/types"
import type { TerminalRenderer } from "@/lib/appConfig"
import type { TerminalAttentionEvent } from "@/lib/terminalNotifications"

export type UseTerminalLifecycleOptions = {
  activateFitTimerRef: React.RefObject<number | null>
  commandMarksRef: React.RefObject<CommandMarks | null>
  commandMarksEnabledRef: React.RefObject<boolean>
  connectionRef: React.RefObject<TerminalTabProps["connection"]>
  containerRef: React.RefObject<HTMLDivElement | null>
  creatingPtyRef: React.RefObject<boolean>
  fitAddonRef: React.RefObject<FitAddon | null>
  fitTerminalOnly: () => boolean
  initializedRef: React.RefObject<boolean>
  configCursorStyleRef?: React.RefObject<Terminal["options"]["cursorStyle"]>
  /** macOS: Option+key sends Meta instead of the alternate character. */
  configMacOptionIsMetaRef?: React.RefObject<boolean>
  /** Programs may copy to the clipboard with OSC 52. */
  configOsc52ClipboardWriteRef?: React.RefObject<boolean>
  configFontFamilyRef?: React.RefObject<string>
  configFontSizeRef?: React.RefObject<number>
  configLineHeightRef?: React.RefObject<number>
  configLetterSpacingRef?: React.RefObject<number>
  configScrollbackLinesRef?: React.RefObject<number>
  configTerminalRendererRef?: React.RefObject<TerminalRenderer>
  terminalThemeRef?: React.RefObject<NonNullable<Terminal["options"]["theme"]>>
  initialCursorStyle?: React.RefObject<Terminal["options"]["cursorStyle"]>
  initialFontFamily?: React.RefObject<string>
  initialFontSize?: React.RefObject<number>
  initialScrollbackLines?: React.RefObject<number>
  initialTerminalRenderer?: React.RefObject<TerminalRenderer>
  initialTerminalThemeRef?: React.RefObject<NonNullable<Terminal["options"]["theme"]>>
  terminalRenderer?: TerminalRenderer
  /**
   * Seconds a terminal stays out of sight before its renderer is released, so
   * switching back and forth between tabs keeps it and a tab left in the
   * background gives back its canvases; 0 keeps it.
   */
  hiddenRendererReleaseSecs: number
  isActive: boolean
  isActiveRef: React.RefObject<boolean>
  lastPtySizeRef: React.RefObject<{ rows: number; cols: number } | null>
  onPidChangeRef: React.RefObject<TerminalTabProps["onPidChange"]>
  onCwdChangeRef: React.RefObject<TerminalTabProps["onCwdChange"]>
  onDistroChangeRef: React.RefObject<TerminalTabProps["onDistroChange"]>
  onInputRef: React.RefObject<TerminalTabProps["onInput"]>
  onCommandExecutedRef: React.RefObject<TerminalTabProps["onCommandExecuted"]>
  onReconnectRequestRef: React.RefObject<TerminalTabProps["onReconnectRequest"]>
  onSavedPasswordPromptChangeRef: React.RefObject<TerminalTabProps["onSavedPasswordPromptChange"]>
  onSessionUnavailableRef: React.RefObject<TerminalTabProps["onSessionUnavailable"]>
  onSensitivePromptRef: React.RefObject<TerminalTabProps["onSensitivePrompt"]>
  onConnectionProgressRef: React.RefObject<TerminalTabProps["onConnectionProgress"]>
  /** A finished command, a notification request, an agent's report or a bell. */
  onAttentionRef: React.RefObject<(event: TerminalAttentionEvent) => void>
  /** Output arrived while the tab was out of sight. */
  onBackgroundOutputRef: React.RefObject<() => void>
  savedPasswordPromptActionsRef: React.RefObject<SavedPasswordPromptActions | null>
  setSavedPasswordPrompt: (value: SavedPasswordPromptState | null) => void
  sudoPromptPatternsRef: React.RefObject<readonly RegExp[]>
  resizeObserverRef: React.RefObject<ResizeObserver | null>
  resizePtySyncTimerRef: React.RefObject<number | null>
  resizeRafRef: React.RefObject<number | null>
  scheduleFitDuringResize: () => void
  surfaceRef: React.RefObject<HTMLDivElement | null>
  searchAddonRef: React.RefObject<SearchAddon | null>
  keywordHighlighterRef: React.RefObject<KeywordHighlighter | null>
  searchResultsDisposableRef: React.RefObject<IDisposable | null>
  setConnectionState: (value: ConnectionState) => void
  setHostKeyPrompt: (value: HostKeyPromptState | null) => void
  setConnectionProgress: (value: SshConnectionProgress | null) => void
  setSearchResults: React.Dispatch<React.SetStateAction<ISearchResultChangeEvent>>
  sessionNonce: number
  tabId: string
  termRef: React.RefObject<Terminal | null>
  waitingForReconnectRef: React.RefObject<boolean>
}

/** State the handlers of one terminal session share. */
export interface TerminalSessionState {
  /** Set once the session is torn down; late callbacks check it and stop. */
  disposed: boolean
  passwordPromptCheckId: number
  commandCaptureState: TerminalCommandCaptureState
  commandCaptureSuspended: boolean
  // Line feeds seen so far: a prompt printed again after a newline (sudo
  // retrying) gets a new key even when it lands on the same screen row.
  lineFeedCount: number
  // Key of the password prompt at the cursor, so each prompt is offered once.
  currentPromptKey: string | null
  lastPasswordFill: { prompt: string; at: number } | null
  // Types the connection's login script; restarted on every (re)connect.
  loginScriptRunner: LoginScriptRunner | null
}

/** What the parts of a terminal session are set up with. */
export type TerminalSessionContext = Pick<
  UseTerminalLifecycleOptions,
  | "commandMarksEnabledRef"
  | "commandMarksRef"
  | "connectionRef"
  | "creatingPtyRef"
  | "fitTerminalOnly"
  | "isActiveRef"
  | "lastPtySizeRef"
  | "onAttentionRef"
  | "onBackgroundOutputRef"
  | "onCommandExecutedRef"
  | "onConnectionProgressRef"
  | "onCwdChangeRef"
  | "onDistroChangeRef"
  | "onInputRef"
  | "onPidChangeRef"
  | "onReconnectRequestRef"
  | "onSensitivePromptRef"
  | "onSessionUnavailableRef"
  | "savedPasswordPromptActionsRef"
  | "sessionNonce"
  | "setConnectionProgress"
  | "setHostKeyPrompt"
  | "setSavedPasswordPrompt"
  | "sudoPromptPatternsRef"
  | "tabId"
  | "waitingForReconnectRef"
> & {
  term: Terminal
  session: TerminalSessionState
  /** The prompt-change callback as it was when the session started. */
  onSavedPasswordPromptChange: TerminalTabProps["onSavedPasswordPromptChange"]
  translationRef: React.RefObject<TFunction>
  scrollbackMemory: ReturnType<typeof createScrollbackMemory>
  savedContent: TrackedTerminalContent
}
