import React from "react"
import { useTranslation } from "react-i18next"

import {
  jumpHostFromProfile,
  profileAddress,
  type JumpHostCandidateGroup,
} from "@/components/ConnectionDialog/jumpHostSource"
import { Label } from "@/components/ui/label"
import { SearchableSelect, type SearchableSelectGroup } from "@/components/ui/searchable-select"
import { profileSearchFields } from "@/lib/profileSearch"
import type { JumpHostForm } from "@/components/ConnectionDialog/types"

interface JumpHostSourceFieldProps {
  jump: JumpHostForm
  candidates: JumpHostCandidateGroup[]
  onChange: (patch: Partial<JumpHostForm>) => void
}

/**
 * Whether a hop is typed in or logs in as one of the saved connections,
 * picked from a searchable list since there can be many of them.
 */
export const JumpHostSourceField: React.FC<JumpHostSourceFieldProps> = ({
  jump,
  candidates,
  onChange,
}) => {
  const { t } = useTranslation()
  const profiles = candidates.flatMap((group) => group.profiles)
  const selected = profiles.find((profile) => profile.id === jump.profileId)
  const missing = Boolean(jump.profileId) && !selected
  const manualLabel = t("jumpHost.sourceManual")

  const groups: SearchableSelectGroup[] = [
    { label: null, options: [{ value: "", label: manualLabel }] },
    ...candidates.map(({ group, profiles: members }) => ({
      label: group || t("jumpHost.sourceSaved"),
      options: members.map((profile) => ({
        value: profile.id,
        label: profile.name,
        detail: profileAddress(profile),
        keywords: profileSearchFields(profile),
      })),
    })),
  ]

  const triggerLabel = selected
    ? `${selected.name} (${profileAddress(selected)})`
    : missing
      ? t("jumpHost.savedConnectionDeleted", {
          address: `${jump.username}@${jump.host}:${jump.port}`,
        })
      : manualLabel

  return (
    <div>
      <Label htmlFor={`jump-source-${jump.id}`} className="mb-1.5 block">
        {t("jumpHost.source")}
      </Label>
      <SearchableSelect
        id={`jump-source-${jump.id}`}
        value={jump.profileId}
        groups={groups}
        onChange={(profileId) => {
          if (profileId === jump.profileId) return
          const profile = profiles.find((candidate) => candidate.id === profileId)
          // Typing it in starts from the copy of the connection it used.
          onChange(profile ? jumpHostFromProfile(profile) : { profileId: "" })
        }}
        triggerLabel={triggerLabel}
        searchLabel={t("jumpHost.sourceSearch")}
        listLabel={t("jumpHost.source")}
        emptyText={t("jumpHost.sourceNoMatch")}
      />
      {selected && (
        <p className="text-muted-foreground mt-1 text-xs">
          {t("jumpHost.savedConnectionHint")}
          {(selected.use_jump_host ?? (selected.jump_hosts?.length ?? 0) > 0) &&
            ` ${t("jumpHost.savedConnectionOwnJumps")}`}
        </p>
      )}
      {missing && (
        <p className="text-destructive mt-1 text-xs" role="alert">
          {t("jumpHost.savedConnectionMissing")}
        </p>
      )}
    </div>
  )
}
