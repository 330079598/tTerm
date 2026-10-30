import { useEffect } from "react"

import { inputComposed } from "@/components/TerminalTab/terminalInput"
import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import { registerComposerTarget } from "@/lib/commandComposer"

type UseComposerTargetOptions = { tabId: string } & Pick<
  TerminalTabRefs,
  "termRef" | "commandMarksRef"
>

/** Lets the compose bar send to this tab's terminal and follow its shell's prompts. */
export function useComposerTarget({ tabId, termRef, commandMarksRef }: UseComposerTargetOptions) {
  useEffect(
    () =>
      registerComposerTarget(tabId, {
        input: (data) => {
          if (termRef.current) inputComposed(termRef.current, data)
        },
        bracketedPasteMode: () => termRef.current?.modes.bracketedPasteMode ?? false,
        promptState: () => commandMarksRef.current?.promptState() ?? "unmarked",
        onPrompt: (listener) => {
          const marks = commandMarksRef.current
          if (marks) return marks.onPrompt(listener)
          // Without marks no prompt will be told, so the wait is over now.
          listener({ kind: "ended" })
          return () => {}
        },
      }),
    [commandMarksRef, tabId, termRef]
  )
}
