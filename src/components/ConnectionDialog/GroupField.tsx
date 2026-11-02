import React, { useState } from "react"
import { useTranslation } from "react-i18next"

import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface GroupFieldProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  /** The existing groups' paths that match what is typed, in tree order. */
  matchingGroups: string[]
}

/** The group a connection is filed in: typed as a path, or picked from the existing ones. */
export const GroupField: React.FC<GroupFieldProps> = ({ form, setForm, matchingGroups }) => {
  const { t } = useTranslation()
  const [showDropdown, setShowDropdown] = useState(false)

  return (
    <div className="relative">
      <Label htmlFor="conn-group" className="mb-1.5 block">
        {t("connection.group")}
      </Label>
      <Input
        id="conn-group"
        value={form.group}
        onChange={(e) => {
          setForm((current) => ({ ...current, group: e.target.value }))
          setShowDropdown(true)
        }}
        onFocus={() => setShowDropdown(true)}
        onBlur={() => setTimeout(() => setShowDropdown(false), 150)}
        placeholder={t("connection.groupPlaceholder")}
        autoComplete="off"
      />
      {showDropdown && matchingGroups.length > 0 && (
        <Card className="absolute top-full right-0 left-0 z-20 mt-1 max-h-40 overflow-y-auto rounded-md">
          <CardContent className="p-1">
            {matchingGroups.map((group) => (
              <button
                key={group}
                type="button"
                className="hover:bg-muted w-full rounded-sm px-3 py-1.5 text-left text-sm"
                onMouseDown={() => {
                  setForm((current) => ({ ...current, group }))
                  setShowDropdown(false)
                }}
              >
                {group}
              </button>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  )
}
