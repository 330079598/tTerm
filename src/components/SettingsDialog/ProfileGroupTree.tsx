import React from "react"

import { GroupDropHint, ProfileGroupCard } from "@/components/SettingsDialog/ProfileGroupCard"
import { UNGROUPED_KEY } from "@/components/SettingsDialog/profileGroupDnd"
import { groupAndDescendantIds, type GroupNode, type GroupTree } from "@/lib/profileGroups"
import type { ProfileGroup, SavedProfile } from "@/types/tab"

interface GroupActions {
  collapsedKeys: ReadonlySet<string>
  groupBusy: boolean
  onToggleCollapsed: (groupKey: string) => void
  onAddGroup: (name: string, parentId: string | null) => Promise<boolean>
  onRenameGroup: (group: ProfileGroup, name: string) => Promise<boolean>
  onDeleteGroup: (node: GroupNode) => void
  renderProfile: (profile: SavedProfile, dropDisabled: boolean) => React.ReactNode
  /** The dragged group and the groups inside it, which take no drops. */
  blockedKeys: ReadonlySet<string>
}

interface ProfileGroupTreeProps extends Omit<GroupActions, "blockedKeys"> {
  groupTree: GroupTree
  /** The group being dragged; it and the groups inside it take no drops. */
  draggingGroupId: string | null
}

const ProfileList: React.FC<{
  profiles: SavedProfile[]
  dropDisabled: boolean
  renderProfile: GroupActions["renderProfile"]
}> = ({ profiles, dropDisabled, renderProfile }) => (
  <div className="space-y-2" role="list">
    {profiles.map((profile) => renderProfile(profile, dropDisabled))}
  </div>
)

/** A group's card, with the groups inside it indented above its own connections. */
const GroupNodeCard: React.FC<GroupActions & { node: GroupNode }> = ({ node, ...actions }) => {
  const { group, children, profiles } = node
  const dropDisabled = actions.blockedKeys.has(group.id)
  return (
    <ProfileGroupCard
      group={group}
      count={node.total}
      groupBusy={actions.groupBusy}
      isCollapsed={actions.collapsedKeys.has(group.id)}
      dropDisabled={dropDisabled}
      onToggleCollapsed={actions.onToggleCollapsed}
      onAddSubgroup={(name) => actions.onAddGroup(name, group.id)}
      onRename={(name) => actions.onRenameGroup(group, name)}
      onDelete={() => actions.onDeleteGroup(node)}
    >
      {children.length > 0 && (
        <div className="border-border/70 ml-1 space-y-2 border-l pl-3">
          {children.map((child) => (
            <GroupNodeCard key={child.group.id} node={child} {...actions} />
          ))}
        </div>
      )}
      {profiles.length > 0 ? (
        <ProfileList
          profiles={profiles}
          dropDisabled={dropDisabled}
          renderProfile={actions.renderProfile}
        />
      ) : (
        children.length === 0 && <GroupDropHint groupKey={group.id} disabled={dropDisabled} />
      )}
    </ProfileGroupCard>
  )
}

/** Every group as a tree of cards, then the connections in no group. */
export const ProfileGroupTree: React.FC<ProfileGroupTreeProps> = ({
  groupTree,
  draggingGroupId,
  ...props
}) => {
  const blockedKeys = React.useMemo(
    () => (draggingGroupId ? groupAndDescendantIds(groupTree, draggingGroupId) : new Set<string>()),
    [draggingGroupId, groupTree]
  )
  const actions = { ...props, blockedKeys }
  return (
    <div className="space-y-3">
      {groupTree.roots.map((node) => (
        <GroupNodeCard key={node.group.id} node={node} {...actions} />
      ))}
      <ProfileGroupCard
        count={groupTree.ungrouped.length}
        groupBusy={actions.groupBusy}
        isCollapsed={actions.collapsedKeys.has(UNGROUPED_KEY)}
        onToggleCollapsed={actions.onToggleCollapsed}
      >
        {groupTree.ungrouped.length > 0 ? (
          <ProfileList
            profiles={groupTree.ungrouped}
            dropDisabled={false}
            renderProfile={actions.renderProfile}
          />
        ) : (
          <GroupDropHint groupKey={UNGROUPED_KEY} />
        )}
      </ProfileGroupCard>
    </div>
  )
}
