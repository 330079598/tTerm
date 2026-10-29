import { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

/** What checking a shell's path found: nothing yet, the program it starts, or nothing there. */
export type ShellPathCheck =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "found"; program: string }
  | { status: "missing" }

// Long enough that typing a path doesn't check every letter of it.
const CHECK_DELAY_MS = 400

/** Checks, once typing pauses, which program a shell's path starts on this machine. */
export function useShellPathCheck(path: string): ShellPathCheck {
  const trimmed = path.trim()
  // The last answer, for the path it answers; another path is still being checked.
  const [answer, setAnswer] = useState<{ path: string; program: string | null } | null>(null)

  useEffect(() => {
    if (!trimmed) return
    let current = true
    const timer = window.setTimeout(() => {
      invoke<string>("check_local_shell", { path: trimmed }).then(
        (program) => {
          if (current) setAnswer({ path: trimmed, program })
        },
        () => {
          if (current) setAnswer({ path: trimmed, program: null })
        }
      )
    }, CHECK_DELAY_MS)
    return () => {
      current = false
      window.clearTimeout(timer)
    }
  }, [trimmed])

  if (!trimmed) return { status: "idle" }
  if (answer?.path !== trimmed) return { status: "checking" }
  return answer.program === null
    ? { status: "missing" }
    : { status: "found", program: answer.program }
}
