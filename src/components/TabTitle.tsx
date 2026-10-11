import { commandTitle } from "@/lib/localTabTitle"
import { useTabRunningCommand } from "@/lib/tabAttention"
import type { Tab } from "@/types/tab"

/**
 * A tab's title. A local tab named automatically shows the command its shell
 * runs instead, for as long as it runs.
 */
export function TabTitle({ tab, className }: { tab: Tab; className: string }) {
  const command = useTabRunningCommand(tab.id)
  const title = (tab.autoTitle && command && commandTitle(command)) || tab.title
  return <span className={className}>{title}</span>
}
