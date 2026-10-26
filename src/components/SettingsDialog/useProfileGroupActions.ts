import React from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useToast } from "@/hooks/use-toast"
import { GROUP_PATH_SEPARATOR, type GroupNode, type GroupTree } from "@/lib/profileGroups"
import { toErrorMessage } from "@/lib/utils"
import type { ProfileGroup, SavedProfile } from "@/types/tab"

interface ProfileGroupActionsOptions {
  groupTree: GroupTree
  setProfileGroups: React.Dispatch<React.SetStateAction<ProfileGroup[]>>
  setProfiles: React.Dispatch<React.SetStateAction<SavedProfile[]>>
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

/** Creating, renaming, moving and deleting profile groups. */
export function useProfileGroupActions({
  groupTree,
  setProfileGroups,
  setProfiles,
  confirm,
}: ProfileGroupActionsOptions) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [groupBusy, setGroupBusy] = React.useState(false)

  /** Why `name` can't name a group under `parentId`, or null if it can. */
  const nameProblem = React.useCallback(
    (name: string, parentId: string | null, exceptId?: string) => {
      if (name.includes(GROUP_PATH_SEPARATOR)) {
        return t("profileGroups.nameHasSeparator", {
          separator: GROUP_PATH_SEPARATOR,
          defaultValue: 'A group name cannot contain "{{separator}}".',
        })
      }
      const siblings = parentId ? (groupTree.nodes.get(parentId)?.children ?? []) : groupTree.roots
      if (siblings.some((node) => node.group.name === name && node.group.id !== exceptId)) {
        return t("profileGroups.duplicate", { defaultValue: "Group already exists" })
      }
      return null
    },
    [groupTree, t]
  )

  /** Runs a group command and takes the groups it returns; false if it failed. */
  const run = React.useCallback(
    async (command: string, args: Record<string, unknown>) => {
      setGroupBusy(true)
      try {
        setProfileGroups(await invoke<ProfileGroup[]>(command, args))
        return true
      } catch (error) {
        toast({
          title: t("profileGroups.saveFailed", { defaultValue: "Failed to save group" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
        return false
      } finally {
        setGroupBusy(false)
      }
    },
    [setProfileGroups, t, toast]
  )

  const checkName = React.useCallback(
    (name: string, parentId: string | null, exceptId?: string) => {
      const problem = nameProblem(name, parentId, exceptId)
      if (problem) toast({ title: problem, variant: "destructive" })
      return problem === null
    },
    [nameProblem, toast]
  )

  const addGroup = React.useCallback(
    async (rawName: string, parentId: string | null) => {
      const name = rawName.trim()
      if (!name || !checkName(name, parentId)) return false
      return run("create_profile_group", { name, parentId })
    },
    [checkName, run]
  )

  const renameGroup = React.useCallback(
    async (group: ProfileGroup, rawName: string) => {
      const name = rawName.trim()
      if (!name || name === group.name) return true
      if (!checkName(name, group.parentId, group.id)) return false
      return run("rename_profile_group", { id: group.id, name })
    },
    [checkName, run]
  )

  const moveGroup = React.useCallback(
    async (group: ProfileGroup, parentId: string | null) => {
      if (group.parentId === parentId || !checkName(group.name, parentId, group.id)) return
      await run("move_profile_group", { id: group.id, parentId })
    },
    [checkName, run]
  )

  const deleteGroup = React.useCallback(
    async (node: GroupNode) => {
      const parent = node.group.parentId ? groupTree.nodes.get(node.group.parentId) : undefined
      const count = node.profiles.length
      const confirmed = await confirm({
        title: t("profileGroups.delete", { defaultValue: "Delete group" }),
        description: parent
          ? t("profileGroups.deleteConfirmNested", {
              count,
              name: node.group.name,
              parent: parent.path,
              defaultValue:
                'Delete "{{name}}"? Its {{count}} saved connections and its groups will move to "{{parent}}".',
            })
          : t("profileGroups.deleteConfirm", {
              count,
              name: node.group.name,
              defaultValue:
                'Delete "{{name}}"? Its {{count}} saved connections will move to Ungrouped, and its groups to the top.',
            }),
        confirmText: t("profileGroups.delete", { defaultValue: "Delete group" }),
        cancelText: t("common.cancel"),
        variant: "destructive",
      })
      if (!confirmed) return

      if (await run("delete_profile_group", { id: node.group.id })) {
        const parentId = node.group.parentId ?? undefined
        setProfiles((current) =>
          current.map((profile) =>
            profile.group_id === node.group.id ? { ...profile, group_id: parentId } : profile
          )
        )
      }
    },
    [confirm, groupTree, run, setProfiles, t]
  )

  return { groupBusy, addGroup, renameGroup, moveGroup, deleteGroup }
}
