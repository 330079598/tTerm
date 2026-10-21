import React from "react"
import { useTranslation } from "react-i18next"

import type { ConnectionForm } from "@/components/ConnectionDialog/types"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import {
  HIGHLIGHT_SET_FOLLOW_DEFAULT,
  HIGHLIGHT_SET_OFF,
  highlightSetName,
  highlightSets,
} from "@/lib/keywordHighlight/config"

interface HighlightSetFieldProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

/** Which keyword highlight set colors the connection's output, or none. */
export const HighlightSetField: React.FC<HighlightSetFieldProps> = ({ form, setForm }) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const sets = highlightSets(config.keyword_highlight)
  const defaultSet = sets.find((set) => set.id === config.keyword_highlight.default_set) ?? sets[0]
  // A set deleted since keeps its place until another choice is made, and
  // falls back to the default set meanwhile.
  const known =
    form.highlightSet === HIGHLIGHT_SET_FOLLOW_DEFAULT ||
    form.highlightSet === HIGHLIGHT_SET_OFF ||
    sets.some((set) => set.id === form.highlightSet)

  return (
    <div>
      <Label htmlFor="conn-highlight-set" className="mb-1.5 block">
        {t("connection.highlightSet")}
      </Label>
      <Select
        id="conn-highlight-set"
        value={known ? form.highlightSet : HIGHLIGHT_SET_FOLLOW_DEFAULT}
        onChange={(event) => {
          const highlightSet = event.target.value
          setForm((current) => ({ ...current, highlightSet }))
        }}
      >
        <option value={HIGHLIGHT_SET_FOLLOW_DEFAULT}>
          {t("connection.highlightSetDefault", {
            name: defaultSet ? highlightSetName(defaultSet, t) : "",
          })}
        </option>
        {sets.map((set) => (
          <option key={set.id} value={set.id}>
            {highlightSetName(set, t)}
          </option>
        ))}
        <option value={HIGHLIGHT_SET_OFF}>{t("connection.highlightSetOff")}</option>
      </Select>
      <p className="text-muted-foreground mt-1 text-xs">{t("connection.highlightSetDesc")}</p>
    </div>
  )
}
