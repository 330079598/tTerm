import React from "react"
import { useTranslation } from "react-i18next"
import { ChevronDown, ChevronRight } from "lucide-react"

import type { GroupNode } from "@/lib/profileGroups"
import type { SavedProfile } from "@/types/tab"

const getGroupListId = (key: string) => `profiles-group-${encodeURIComponent(key)}`

interface GroupListProps {
  groupKey: string
  label: string
  count: number
  isCollapsed: boolean
  onToggle: (key: string) => void
  children: React.ReactNode
}

/** A collapsible heading with what is filed under it. */
export const GroupList: React.FC<GroupListProps> = ({
  groupKey,
  label,
  count,
  isCollapsed,
  onToggle,
  children,
}) => {
  const { t } = useTranslation()
  const listId = getGroupListId(groupKey)

  return (
    <section className="space-y-2">
      <button
        type="button"
        className="text-muted-foreground hover:bg-muted/35 focus-visible:ring-ring/50 flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left text-[11px] font-semibold tracking-[0.08em] transition-colors outline-none focus-visible:ring-[3px]"
        onClick={() => onToggle(groupKey)}
        aria-expanded={!isCollapsed}
        aria-controls={listId}
        aria-label={
          isCollapsed
            ? t("profiles.expandGroup", { group: label })
            : t("profiles.collapseGroup", { group: label })
        }
      >
        {isCollapsed ? (
          <ChevronRight className="size-3 shrink-0" />
        ) : (
          <ChevronDown className="size-3 shrink-0" />
        )}
        <span className="min-w-0 flex-1 whitespace-pre-wrap">{label}</span>
        <span className="shrink-0 tabular-nums">{count}</span>
      </button>

      {!isCollapsed && (
        <div id={listId} className="space-y-2">
          {children}
        </div>
      )}
    </section>
  )
}

interface ProfileGroupSectionProps {
  node: GroupNode
  collapsedKeys: ReadonlySet<string>
  onToggle: (key: string) => void
  renderRow: (profile: SavedProfile) => React.ReactNode
}

/**
 * A group with the groups inside it, indented under its heading, and then
 * its own profiles. Groups holding no profile at any depth are left out.
 */
export const ProfileGroupSection: React.FC<ProfileGroupSectionProps> = ({
  node,
  collapsedKeys,
  onToggle,
  renderRow,
}) => {
  const children = node.children.filter((child) => child.total > 0)

  return (
    <GroupList
      groupKey={node.group.id}
      label={node.group.name}
      count={node.total}
      isCollapsed={collapsedKeys.has(node.group.id)}
      onToggle={onToggle}
    >
      {children.length > 0 && (
        <div className="border-border/70 ml-1.5 space-y-2 border-l pl-2">
          {children.map((child) => (
            <ProfileGroupSection
              key={child.group.id}
              node={child}
              collapsedKeys={collapsedKeys}
              onToggle={onToggle}
              renderRow={renderRow}
            />
          ))}
        </div>
      )}
      {node.profiles.length > 0 && (
        <div className="space-y-2" role="listbox" aria-label={node.path}>
          {node.profiles.map(renderRow)}
        </div>
      )}
    </GroupList>
  )
}
