import { useTranslation } from "react-i18next"
import { RotateCcw, Trash2, X } from "lucide-react"

import { formatSavedAt } from "@/components/TerminalTab/terminalSnapshot"
import { TabIconGlyph } from "@/components/TabIcon"
import { Button } from "@/components/ui/button"
import { useConfig } from "@/contexts/ConfigContext"
import type { ClosedTab } from "@/lib/closedTabs"
import type { CollapsedGroup, CollapsedSet } from "@/lib/collapsedTabs"
import { hasLocalBadge, resolveTabIcon } from "@/lib/tabIcons"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"

interface CollapsedSetItemProps {
  set: CollapsedSet
  onRestoreGroup: (setId: string, group: CollapsedGroup) => void
  onRestoreSet: (set: CollapsedSet) => void
  onDeleteGroup: (setId: string, groupId: string) => void
  onDeleteSet: (setId: string) => void
}

/** What a collapsed tab reached: the host of a connection, the directory of a local shell. */
function tabDetail(tab: ClosedTab): string | undefined {
  const connection = tab.connection
  if (!connection) return undefined
  if (tab.type === "ssh" && connection.host) {
    const host =
      connection.port && connection.port !== 22
        ? `${connection.host}:${connection.port}`
        : connection.host
    return connection.username ? `${connection.username}@${host}` : host
  }
  return connection.cwd?.trim() || undefined
}

function CollapsedTabIcon({ tab }: { tab: ClosedTab }) {
  const { config } = useConfig()
  const icon = resolveTabIcon(tab, config.tab_icons)
  return (
    <span className="flex size-4 shrink-0 items-center justify-center">
      {icon && <TabIconGlyph icon={icon} localBadge={hasLocalBadge(tab, icon)} />}
    </span>
  )
}

/** One tab of the bar put away: its panes' titles, and where the first one reached. */
function CollapsedGroupRow({
  group,
  onRestore,
  onDelete,
}: {
  group: CollapsedGroup
  onRestore: () => void
  onDelete: () => void
}) {
  const { t } = useTranslation()
  const [first] = group.tabs
  const title = group.tabs.map((tab) => tab.title).join(" | ")
  const detail = tabDetail(group.tabs[group.activeIndex] ?? first)
  return (
    <li className="group/row hover:bg-accent flex items-center gap-1 rounded-md pr-1">
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2 px-2 py-1.5 text-left"
        // The focus stays in the terminal, and moves to the reopened one as it opens.
        onMouseDown={keepFocusOnMouseDown}
        onClick={onRestore}
        title={t("collapsedTabs.restoreTab")}
      >
        <CollapsedTabIcon tab={first} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm">{title}</span>
          {detail && <span className="text-muted-foreground block truncate text-xs">{detail}</span>}
        </span>
        {group.tabs.length > 1 && (
          <span className="text-muted-foreground shrink-0 text-xs">
            {t("workspace.paneCount", { count: group.tabs.length })}
          </span>
        )}
      </button>
      <Button
        variant="ghost"
        size="icon-xs"
        className="opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100"
        onClick={onDelete}
        aria-label={t("collapsedTabs.deleteTab")}
        title={t("collapsedTabs.deleteTab")}
      >
        <X className="size-3" />
      </Button>
    </li>
  )
}

/** The tabs put away by one click, which open again one by one or all together. */
export function CollapsedSetItem({
  set,
  onRestoreGroup,
  onRestoreSet,
  onDeleteGroup,
  onDeleteSet,
}: CollapsedSetItemProps) {
  const { t } = useTranslation()
  return (
    <section className="border-border border-b px-2 py-2 last:border-b-0">
      <header className="flex items-center gap-1 px-2 pb-1">
        <span className="text-muted-foreground min-w-0 flex-1 truncate text-xs">
          {t("collapsedTabs.setTitle", {
            count: set.groups.length,
            time: formatSavedAt(set.collapsedAt),
          })}
        </span>
        <Button
          variant="ghost"
          size="xs"
          onMouseDown={keepFocusOnMouseDown}
          onClick={() => onRestoreSet(set)}
        >
          <RotateCcw className="size-3" />
          {t("collapsedTabs.restoreAll")}
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={() => onDeleteSet(set.id)}
          aria-label={t("collapsedTabs.deleteSet")}
          title={t("collapsedTabs.deleteSet")}
        >
          <Trash2 className="size-3" />
        </Button>
      </header>
      <ul>
        {set.groups.map((group) => (
          <CollapsedGroupRow
            key={group.id}
            group={group}
            onRestore={() => onRestoreGroup(set.id, group)}
            onDelete={() => onDeleteGroup(set.id, group.id)}
          />
        ))}
      </ul>
    </section>
  )
}
