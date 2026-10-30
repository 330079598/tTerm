import React from "react"
import { useTranslation } from "react-i18next"

import {
  jumpHostFromProfile,
  profileAddress,
  type JumpHostCandidateGroup,
} from "@/components/ConnectionDialog/jumpHostSource"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import type { JumpHostForm } from "@/components/ConnectionDialog/types"
import type { SavedProfile } from "@/types/tab"

interface JumpHostSourceFieldProps {
  jump: JumpHostForm
  candidates: JumpHostCandidateGroup[]
  onChange: (patch: Partial<JumpHostForm>) => void
}

/** Whether a hop is typed in or logs in as one of the saved connections. */
export const JumpHostSourceField: React.FC<JumpHostSourceFieldProps> = ({
  jump,
  candidates,
  onChange,
}) => {
  const { t } = useTranslation()
  const profiles = candidates.flatMap((group) => group.profiles)
  const selected = profiles.find((profile) => profile.id === jump.profileId)
  const missing = Boolean(jump.profileId) && !selected
  const optionLabel = (profile: SavedProfile) => `${profile.name} (${profileAddress(profile)})`

  return (
    <div>
      <Label htmlFor={`jump-source-${jump.id}`} className="mb-1.5 block">
        {t("jumpHost.source")}
      </Label>
      <Select
        id={`jump-source-${jump.id}`}
        value={jump.profileId}
        onChange={(e) => {
          const profile = profiles.find((candidate) => candidate.id === e.target.value)
          // Typing it in starts from the copy of the connection it used.
          onChange(profile ? jumpHostFromProfile(profile) : { profileId: "" })
        }}
      >
        <option value="">{t("jumpHost.sourceManual")}</option>
        {missing && (
          <option value={jump.profileId} disabled>
            {t("jumpHost.savedConnectionDeleted", {
              address: `${jump.username}@${jump.host}:${jump.port}`,
            })}
          </option>
        )}
        {candidates.map(({ group, profiles: members }) => (
          <optgroup key={group} label={group || t("jumpHost.sourceSaved")}>
            {members.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {optionLabel(profile)}
              </option>
            ))}
          </optgroup>
        ))}
      </Select>
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
