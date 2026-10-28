import { quotePosix } from "@/lib/droppedPaths"

/**
 * Why the terminal was not sent a `cd`: a program has the line, or the shell
 * runs on another host reached with ssh, where the path means something else.
 */
export type CdRefusal = { reason: "busy" } | { reason: "elsewhere"; host: string }

/**
 * The line that takes a remote shell to `path`, Enter included. The path is
 * escaped as for a dropped file, which bash, zsh and fish all read.
 */
export function terminalCdCommand(path: string): string {
  return `cd ${quotePosix(path)}\r`
}
