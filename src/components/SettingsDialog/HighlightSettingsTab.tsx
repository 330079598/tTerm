import React, { useState } from "react"
import { Eye, Highlighter, ListChecks, Monitor, Plus, Power } from "lucide-react"
import { useTranslation } from "react-i18next"

import { HighlightPreview } from "@/components/SettingsDialog/HighlightPreview"
import { HighlightRuleEditor } from "@/components/SettingsDialog/HighlightRuleEditor"
import { HighlightRuleList } from "@/components/SettingsDialog/HighlightRuleList"
import { HighlightSetToolbar } from "@/components/SettingsDialog/HighlightSetToolbar"
import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { useHighlightSettings } from "@/components/SettingsDialog/useHighlightSettings"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useConfig } from "@/contexts/ConfigContext"
import { useTheme } from "@/contexts/ThemeContext"
import { highlightSetName, MAX_HIGHLIGHT_RULES } from "@/lib/keywordHighlight/config"
import {
  addHighlightRule,
  moveHighlightRule,
  newHighlightRule,
  removeHighlightRule,
  updateHighlightRule,
} from "@/lib/keywordHighlight/edit"

export const HighlightSettingsTab: React.FC = () => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { displayedTheme, getTheme } = useTheme()
  const colors = getTheme(displayedTheme)?.terminal ?? getTheme("default")?.terminal ?? {}
  const { highlight, sets, edit, editSet } = useHighlightSettings()
  const [editingSetId, setEditingSetId] = useState(highlight.default_set)
  const [selectedRuleId, setSelectedRuleId] = useState<string | null>(null)
  const editingSet =
    sets.find((set) => set.id === editingSetId) ??
    sets.find((set) => set.id === highlight.default_set) ??
    sets[0]
  const selectedRule = editingSet?.rules.find((rule) => rule.id === selectedRuleId)

  const addRule = () => {
    if (!editingSet) return
    const rule = newHighlightRule()
    editSet(editingSet.id, (set) => addHighlightRule(set, rule))
    setSelectedRuleId(rule.id)
  }

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <SettingsSection
          icon={<Highlighter size={16} />}
          title={t("keywordHighlight.title")}
          description={t("keywordHighlight.description")}
        >
          <SettingsRow
            icon={<Power size={16} />}
            title={t("keywordHighlight.enabled")}
            description={t("keywordHighlight.enabledDesc")}
            action={
              <Switch
                checked={highlight.enabled}
                aria-label={t("keywordHighlight.enabled")}
                onCheckedChange={(enabled) => edit((current) => ({ ...current, enabled }))}
              />
            }
          />
          <SettingsRow
            icon={<ListChecks size={16} />}
            title={t("keywordHighlight.defaultSet")}
            description={t("keywordHighlight.defaultSetDesc")}
            action={
              <Select
                aria-label={t("keywordHighlight.defaultSet")}
                value={highlight.default_set}
                className="w-44"
                onChange={(event) => {
                  const defaultSet = event.target.value
                  edit((current) => ({ ...current, default_set: defaultSet }))
                }}
              >
                {sets.map((set) => (
                  <option key={set.id} value={set.id}>
                    {highlightSetName(set, t)}
                  </option>
                ))}
              </Select>
            }
          />
          <SettingsRow
            icon={<Monitor size={16} />}
            title={t("keywordHighlight.alternateScreen")}
            description={t("keywordHighlight.alternateScreenDesc")}
            action={
              <Switch
                checked={highlight.alternate_screen}
                aria-label={t("keywordHighlight.alternateScreen")}
                onCheckedChange={(alternateScreen) =>
                  edit((current) => ({ ...current, alternate_screen: alternateScreen }))
                }
              />
            }
          />
        </SettingsSection>

        {editingSet && (
          <SettingsSection
            icon={<ListChecks size={16} />}
            title={t("keywordHighlight.sets")}
            description={t("keywordHighlight.setsDesc")}
          >
            <HighlightSetToolbar
              sets={sets}
              editingSet={editingSet}
              edit={edit}
              editSet={editSet}
              onEditingSetChange={(setId) => {
                setEditingSetId(setId)
                setSelectedRuleId(null)
              }}
            />
            <HighlightRuleList
              rules={editingSet.rules}
              selectedId={selectedRule?.id ?? null}
              colors={colors}
              onSelect={setSelectedRuleId}
              onToggle={(ruleId, enabled) =>
                editSet(editingSet.id, (set) => updateHighlightRule(set, ruleId, { enabled }))
              }
              onMove={(ruleId, offset) =>
                editSet(editingSet.id, (set) => moveHighlightRule(set, ruleId, offset))
              }
              onRemove={(ruleId) =>
                editSet(editingSet.id, (set) => removeHighlightRule(set, ruleId))
              }
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={editingSet.rules.length >= MAX_HIGHLIGHT_RULES}
              onClick={addRule}
            >
              <Plus />
              {t("keywordHighlight.addRule")}
            </Button>
            {selectedRule && (
              <Card className="shadow-none">
                <CardContent className="p-4">
                  <HighlightRuleEditor
                    key={selectedRule.id}
                    rule={selectedRule}
                    palette={colors}
                    onChange={(changes) =>
                      editSet(editingSet.id, (set) =>
                        updateHighlightRule(set, selectedRule.id, changes)
                      )
                    }
                  />
                </CardContent>
              </Card>
            )}
          </SettingsSection>
        )}

        {editingSet && (
          <SettingsSection
            icon={<Eye size={16} />}
            title={t("keywordHighlight.preview")}
            description={t("keywordHighlight.previewDesc")}
          >
            <HighlightPreview set={editingSet} colors={colors} fontFamily={config.font_family} />
          </SettingsSection>
        )}
      </div>
    </ScrollArea>
  )
}
