import { invoke, type InvokeArgs } from "@tauri-apps/api/core"

import { toast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"

export type ReportErrorOptions = {
  /** Toast title. Defaults to a generic failure title when not silent. */
  title?: string
  /** User-facing description. Falls back to the error message. */
  userMessage?: string
  /** Log-only; no toast. Use for probes, cleanup, and cancelled races. */
  silent?: boolean
  /** Prefix for console logs, e.g. `save_session`. */
  context?: string
}

/**
 * Unified error reporting: always logs; optionally shows a destructive toast.
 */
export function reportError(error: unknown, options: ReportErrorOptions = {}): string {
  const message = toErrorMessage(error)
  const prefix = options.context ? `[${options.context}] ` : ""
  console.error(`${prefix}${message}`, error)

  if (!options.silent) {
    toast({
      title: options.title ?? "Operation failed",
      description: options.userMessage ?? message,
      variant: "destructive",
    })
  }

  return message
}

export type InvokeSafeResult<T> = { ok: true; value: T } | { ok: false; error: unknown }

export type InvokeSafeOptions = Omit<ReportErrorOptions, "silent"> & {
  /** May be evaluated only after an invocation fails. */
  silent?: boolean | (() => boolean)
}

/**
 * `invoke` wrapper that reports failures consistently.
 *
 * A discriminated result keeps a failed invocation distinct from a successful
 * command whose payload happens to be `null`.
 */
export async function invokeSafe<T>(
  cmd: string,
  args?: InvokeArgs,
  options: InvokeSafeOptions = {}
): Promise<InvokeSafeResult<T>> {
  try {
    return { ok: true, value: await invoke<T>(cmd, args) }
  } catch (error) {
    const silent = typeof options.silent === "function" ? options.silent() : options.silent
    reportError(error, {
      ...options,
      silent,
      context: options.context ?? cmd,
    })
    return { ok: false, error }
  }
}
