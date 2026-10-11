import { runningCommandTitle } from "@/lib/localTabTitle"
import { useRunningCommands } from "@/lib/tabAttention"
import type { Tab } from "@/types/tab"

/**
 * A tab's title. A local tab named automatically shows the command its shell
 * runs instead, for as long as it runs.
 */
export function TabTitle({ tab, className }: { tab: Tab; className: string }) {
  const commands = useRunningCommands()
  return <span className={className}>{runningCommandTitle(tab, commands) || tab.title}</span>
}
