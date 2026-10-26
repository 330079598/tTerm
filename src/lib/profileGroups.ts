import type { ProfileGroup, SavedProfile } from "@/types/tab"

/** Separates the names of a group path as typed; a group name cannot hold it. */
export const GROUP_PATH_SEPARATOR = "/"
/** Joins the names of a group path for display; it reads back as the same path. */
const GROUP_PATH_JOINER = " / "

/** A typed group path as the backend reads it: names trimmed, empty ones dropped. */
export function normalizeGroupPath(path: string): string {
  return path
    .split(GROUP_PATH_SEPARATOR)
    .map((name) => name.trim())
    .filter(Boolean)
    .join(GROUP_PATH_JOINER)
}

/** A group in the tree, with the groups and profiles filed in it. */
export interface GroupNode {
  group: ProfileGroup
  /** How many groups it sits in: 0 at the top. */
  depth: number
  /** The names from the top down to this group, joined for display. */
  path: string
  children: GroupNode[]
  profiles: SavedProfile[]
  /** The profiles in this group and in every group inside it. */
  total: number
}

export interface GroupTree {
  roots: GroupNode[]
  /** Profiles in no group, or in a group that no longer exists. */
  ungrouped: SavedProfile[]
  /** Every node by group id. */
  nodes: Map<string, GroupNode>
}

const byName = (left: ProfileGroup, right: ProfileGroup) =>
  left.name.localeCompare(right.name) || left.id.localeCompare(right.id)

/**
 * The groups as a tree, siblings by name, each holding its profiles in the
 * order given. A group whose parent is gone sits at the top, and so does the
 * group with the smallest id in a loop, as the backend mends them.
 */
export function buildGroupTree(
  groups: readonly ProfileGroup[],
  profiles: readonly SavedProfile[]
): GroupTree {
  const known = new Set(groups.map((group) => group.id))
  const children = new Map<string | null, ProfileGroup[]>()
  for (const group of [...groups].sort(byName)) {
    const parent =
      group.parentId && group.parentId !== group.id && known.has(group.parentId)
        ? group.parentId
        : null
    children.set(parent, [...(children.get(parent) ?? []), group])
  }

  const nodes = new Map<string, GroupNode>()
  const place = (group: ProfileGroup, depth: number, parentPath: string): GroupNode => {
    const path = parentPath ? `${parentPath}${GROUP_PATH_JOINER}${group.name}` : group.name
    const node: GroupNode = { group, depth, path, children: [], profiles: [], total: 0 }
    nodes.set(group.id, node)
    node.children = (children.get(group.id) ?? [])
      .filter((child) => !nodes.has(child.id))
      .map((child) => place(child, depth + 1, path))
    return node
  }

  const roots = (children.get(null) ?? []).map((group) => place(group, 0, ""))
  // Groups in a loop are reached from no top-level group.
  for (;;) {
    const cut = groups
      .filter((group) => !nodes.has(group.id))
      .sort((left, right) => left.id.localeCompare(right.id))[0]
    if (!cut) break
    roots.push(place(cut, 0, ""))
  }
  roots.sort((left, right) => byName(left.group, right.group))

  const ungrouped: SavedProfile[] = []
  for (const profile of profiles) {
    const node = profile.group_id ? nodes.get(profile.group_id) : undefined
    if (node) {
      node.profiles.push(profile)
    } else {
      ungrouped.push(profile)
    }
  }
  const count = (node: GroupNode): number => {
    node.total = node.profiles.length + node.children.reduce((sum, child) => sum + count(child), 0)
    return node.total
  }
  roots.forEach(count)

  return { roots, ungrouped, nodes }
}

/** The ids of a group and of every group inside it. */
export function groupAndDescendantIds(tree: GroupTree, groupId: string): Set<string> {
  const ids = new Set<string>()
  const visit = (node: GroupNode) => {
    ids.add(node.group.id)
    node.children.forEach(visit)
  }
  const node = tree.nodes.get(groupId)
  if (node) visit(node)
  return ids
}

/**
 * The groups in the order the tree lists them, a group before the groups in
 * it, skipping what sits in a collapsed group.
 */
export function visibleGroupNodes(
  tree: GroupTree,
  collapsedIds: ReadonlySet<string> = new Set()
): GroupNode[] {
  const result: GroupNode[] = []
  const visit = (node: GroupNode) => {
    result.push(node)
    if (!collapsedIds.has(node.group.id)) node.children.forEach(visit)
  }
  tree.roots.forEach(visit)
  return result
}

/**
 * The profiles in the order the tree shows them: in each group the groups
 * inside it first, then its own profiles; the ungrouped last. Profiles in a
 * collapsed group are left out.
 */
export function visibleTreeProfiles(
  tree: GroupTree,
  collapsedKeys: ReadonlySet<string>,
  ungroupedKey: string
): SavedProfile[] {
  const result: SavedProfile[] = []
  const visit = (node: GroupNode) => {
    if (collapsedKeys.has(node.group.id)) return
    node.children.forEach(visit)
    result.push(...node.profiles)
  }
  tree.roots.forEach(visit)
  if (!collapsedKeys.has(ungroupedKey)) result.push(...tree.ungrouped)
  return result
}

/** The ids of the groups that hold no profiles, in them or in a group inside them. */
export function emptyGroupIds(tree: GroupTree): Set<string> {
  return new Set(
    [...tree.nodes.values()].filter((node) => node.total === 0).map((node) => node.group.id)
  )
}
