import { quotePosix } from "@/lib/droppedPaths"

/**
 * The line that takes a remote shell to `path`, Enter included. The path is
 * escaped as for a dropped file, which bash, zsh and fish all read.
 */
export function terminalCdCommand(path: string): string {
  return `cd ${quotePosix(path)}\r`
}
