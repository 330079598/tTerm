import { FitAddon } from "@xterm/addon-fit"
import { SearchAddon } from "@xterm/addon-search"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { Terminal } from "@xterm/xterm"
import { invoke } from "@tauri-apps/api/core"
import { openUrl } from "@tauri-apps/plugin-opener"
import { platform } from "@tauri-apps/plugin-os"

import type { UseTerminalLifecycleOptions } from "@/components/TerminalTab/terminalLifecycleTypes"
import { installImeCursorGuard } from "@/components/TerminalTab/imeCursorGuard"
import { installImeEarlyInputFix } from "@/components/TerminalTab/imeEarlyInput"
import { installImeFocusRepair } from "@/components/TerminalTab/imeFocusRepair"
import { createScrollbackMemory } from "@/components/TerminalTab/scrollbackMemory"
import { OSC52_CODE, parseOsc52Copy } from "@/lib/osc52"
import { startingScrollbackLines } from "@/lib/scrollback"
import { isTransparentTerminalTheme } from "@/lib/terminalPalette"
import { updateCanvasFontHostFont } from "@/lib/canvasFontHost"

const LINK_MODIFIER_IS_CMD = (() => {
  try {
    return platform() === "macos"
  } catch {
    if (typeof navigator !== "undefined") {
      const platformHint = `${navigator.platform} ${navigator.userAgent}`.toLowerCase()
      if (platformHint.includes("mac")) {
        return true
      }
    }

    return false
  }
})()

// The IME focus loss it repairs is specific to WebView2.
const IS_WINDOWS = (() => {
  try {
    return platform() === "windows"
  } catch {
    return typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent)
  }
})()

function isLinkOpenModifierPressed(event: MouseEvent) {
  return LINK_MODIFIER_IS_CMD ? event.metaKey : event.ctrlKey
}

type OpenTerminalOptions = Pick<
  UseTerminalLifecycleOptions,
  | "configMacOptionIsMetaRef"
  | "configOsc52ClipboardWriteRef"
  | "configLetterSpacingRef"
  | "configLineHeightRef"
  | "termRef"
  | "fitAddonRef"
  | "searchAddonRef"
  | "searchResultsDisposableRef"
  | "setSearchResults"
> & {
  container: HTMLDivElement
  cursorStyleRef: NonNullable<UseTerminalLifecycleOptions["configCursorStyleRef"]>
  scrollbackLinesRef: NonNullable<UseTerminalLifecycleOptions["configScrollbackLinesRef"]>
  fontSizeRef: NonNullable<UseTerminalLifecycleOptions["configFontSizeRef"]>
  fontFamilyRef: NonNullable<UseTerminalLifecycleOptions["configFontFamilyRef"]>
  themeRef: NonNullable<UseTerminalLifecycleOptions["terminalThemeRef"]>
}

/**
 * Creates the terminal with its addons and opens it in `container`, with
 * the fixes the IME and the scrollback need.
 */
export function openTerminal({
  container,
  cursorStyleRef,
  configMacOptionIsMetaRef,
  configOsc52ClipboardWriteRef,
  scrollbackLinesRef,
  fontSizeRef,
  fontFamilyRef,
  configLetterSpacingRef,
  configLineHeightRef,
  themeRef,
  termRef,
  fitAddonRef,
  searchAddonRef,
  searchResultsDisposableRef,
  setSearchResults,
}: OpenTerminalOptions) {
  // Ensure WebKit resolves the local font on the canvas font host and container before warmUp
  updateCanvasFontHostFont(fontFamilyRef.current, fontSizeRef.current)
  container.style.fontFamily = fontFamilyRef.current
  try {
    void container.offsetWidth
  } catch {
    // Ignore in non-browser environments
  }

  const term = new Terminal({
    cursorBlink: true,
    cursorStyle: cursorStyleRef.current,
    macOptionIsMeta: configMacOptionIsMetaRef?.current ?? false,
    scrollback: startingScrollbackLines(scrollbackLinesRef.current),
    fontSize: fontSizeRef.current,
    fontFamily: fontFamilyRef.current,
    fontWeight: "normal",
    fontWeightBold: "bold",
    letterSpacing: configLetterSpacingRef?.current ?? 0,
    lineHeight: configLineHeightRef?.current ?? 1,
    theme: themeRef.current,
    allowTransparency: isTransparentTerminalTheme(themeRef.current),
    allowProposedApi: true,
    // Under the 10px scrollbar (TerminalTab.css): search matches, commands.
    overviewRuler: { width: 10 },
  })

  const fitAddon = new FitAddon()
  const searchAddon = new SearchAddon({ highlightLimit: 2000 })
  term.loadAddon(fitAddon)
  term.loadAddon(searchAddon)
  term.loadAddon(
    new WebLinksAddon((event, uri) => {
      if (!isLinkOpenModifierPressed(event)) {
        return
      }

      event.preventDefault()
      void openUrl(uri).catch((error) => {
        console.error("Failed to open terminal link:", error)
      })
    })
  )
  term.loadAddon(new Unicode11Addon())
  term.unicode.activeVersion = "11"
  // Disposed with the terminal.
  term.parser.registerOscHandler(OSC52_CODE, (data) => {
    if (configOsc52ClipboardWriteRef?.current === false) return true
    const text = parseOsc52Copy(data)
    if (text) {
      invoke("plugin:clipboard-manager|write_text", { text }).catch((error) => {
        console.error("Failed to copy text sent with OSC 52:", error)
      })
    }
    return true
  })

  termRef.current = term
  fitAddonRef.current = fitAddon
  searchAddonRef.current = searchAddon
  searchResultsDisposableRef.current = searchAddon.onDidChangeResults((results) => {
    setSearchResults(results)
  })

  container.replaceChildren()
  term.open(container)
  const imeCursorGuard = installImeCursorGuard(term)
  const imeEarlyInputFix = installImeEarlyInputFix(term)
  const scrollbackMemory = createScrollbackMemory(term, () => scrollbackLinesRef.current)
  const imeFocusRepair = IS_WINDOWS
    ? installImeFocusRepair(term, () => invoke("repair_ime_focus"))
    : null

  return { term, imeCursorGuard, imeEarlyInputFix, scrollbackMemory, imeFocusRepair }
}
