import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef } from "react"

import { useConfig } from "@/contexts/ConfigContext"
import type { KeymapActionId } from "@/lib/keymap/actions"
import { isMacPlatform } from "@/lib/keymap/chord"
import { attachKeymapDispatcher, type KeymapHandler } from "@/lib/keymap/dispatcher"
import { buildChordIndex, resolveEffectiveKeymap, type EffectiveKeymap } from "@/lib/keymap/keymap"

interface KeymapContextValue {
  /** Serialized chords currently in effect per action (null = unbound). */
  bindings: EffectiveKeymap
  /**
   * Register a handler for an action. Multiple handlers per action are
   * allowed (e.g. every visible terminal panel in a split responds to
   * terminal.find). Returns an unregister function.
   */
  registerHandler: (actionId: KeymapActionId, handler: KeymapHandler) => () => void
  /** Pause global dispatching (settings key recorder, ...). */
  setDispatchSuppressed: (suppressed: boolean) => void
}

const KeymapContext = createContext<KeymapContextValue | null>(null)

const MODAL_OPEN_SELECTOR = '[data-slot="dialog-content"][data-state="open"]'

export function KeymapProvider({ children }: { children: React.ReactNode }) {
  const { config } = useConfig()

  const handlersRef = useRef(new Map<KeymapActionId, Set<KeymapHandler>>())
  const suppressedRef = useRef(false)
  const indexRef = useRef(new Map<string, KeymapActionId>())
  const isMacRef = useRef(isMacPlatform())

  const bindings = useMemo<EffectiveKeymap>(
    () => resolveEffectiveKeymap(config.keymap),
    [config.keymap]
  )

  useEffect(() => {
    indexRef.current = buildChordIndex(bindings)
  }, [bindings])

  useEffect(() => {
    return attachKeymapDispatcher(window, {
      isDispatchSuppressed: () => suppressedRef.current,
      hasOpenModal: () =>
        typeof document !== "undefined" && Boolean(document.querySelector(MODAL_OPEN_SELECTOR)),
      getChordIndex: () => indexRef.current,
      getHandlers: () => handlersRef.current,
      isMac: () => isMacRef.current,
    })
  }, [])

  const registerHandler = useCallback((actionId: KeymapActionId, handler: KeymapHandler) => {
    const handlers = handlersRef.current
    let set = handlers.get(actionId)
    if (!set) {
      set = new Set()
      handlers.set(actionId, set)
    }
    set.add(handler)
    return () => {
      const current = handlers.get(actionId)
      if (!current) {
        return
      }
      current.delete(handler)
      if (current.size === 0) {
        handlers.delete(actionId)
      }
    }
  }, [])

  const setDispatchSuppressed = useCallback((suppressed: boolean) => {
    suppressedRef.current = suppressed
  }, [])

  const value = useMemo<KeymapContextValue>(
    () => ({ bindings, registerHandler, setDispatchSuppressed }),
    [bindings, registerHandler, setDispatchSuppressed]
  )

  return <KeymapContext.Provider value={value}>{children}</KeymapContext.Provider>
}

export function useKeymap(): KeymapContextValue {
  const context = useContext(KeymapContext)
  if (!context) {
    throw new Error("useKeymap must be used within a KeymapProvider")
  }
  return context
}
