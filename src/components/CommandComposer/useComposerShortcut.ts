import { useEffect, type RefObject } from "react"

import { useKeymap } from "@/contexts/KeymapContext"
import { closeComposer, openComposer } from "@/lib/commandComposer"
import { focusActiveTerminal } from "@/lib/terminalFocus"

/**
 * The shortcut opens the bar, or brings the keyboard to it, and closes it
 * only from inside it, so it never closes a bar the user meant to reach.
 * Each focus request puts the keyboard in the bar.
 */
export function useComposerShortcut(
  textareaRef: RefObject<HTMLTextAreaElement | null>,
  focusRequest: number
) {
  const { registerHandler } = useKeymap()

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
    [registerHandler, textareaRef]
  )

  useEffect(() => {
    if (focusRequest === 0) return
    // A dialog that asked for the bar, such as the command library, gives the
    // focus back to where it was as it closes; the bar takes it after that.
    const frame = requestAnimationFrame(() => textareaRef.current?.focus())
    return () => cancelAnimationFrame(frame)
  }, [focusRequest, textareaRef])
}
