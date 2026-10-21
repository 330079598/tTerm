import React from "react"
import { useTranslation } from "react-i18next"

import { HighlightColorPicker } from "@/components/SettingsDialog/HighlightColorPicker"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useSyncedState } from "@/hooks/useSyncedState"
import { isImeKeyEvent } from "@/lib/ime"
import type { HighlightRule } from "@/lib/keywordHighlight/config"
import { type HighlightPalette, highlightPatternError } from "@/lib/keywordHighlight/match"

interface HighlightRuleEditorProps {
  rule: HighlightRule
  palette: HighlightPalette
  onChange: (changes: Partial<Omit<HighlightRule, "id">>) => void
}

/**
 * Edits one rule. The pattern is saved when the field loses focus or Enter
 * is pressed, and only once it is valid; the error shows while typing.
 */
export const HighlightRuleEditor: React.FC<HighlightRuleEditorProps> = ({
  rule,
  palette,
  onChange,
}) => {
  const { t } = useTranslation()
  const [pattern, setPattern] = useSyncedState(rule.pattern)
  const error = highlightPatternError({ ...rule, pattern })

  const commitPattern = () => {
    if (pattern !== rule.pattern && !error) onChange({ pattern })
  }

  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <Label htmlFor={`highlight-pattern-${rule.id}`}>{t("keywordHighlight.pattern")}</Label>
        <div className="flex gap-2">
          <Input
            id={`highlight-pattern-${rule.id}`}
            value={pattern}
            placeholder={t(
              rule.kind === "regex"
                ? "keywordHighlight.regexPlaceholder"
                : "keywordHighlight.keywordPlaceholder"
            )}
            aria-invalid={error !== null && error !== "empty"}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            className="font-mono"
            onChange={(event) => setPattern(event.target.value)}
            onBlur={commitPattern}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) commitPattern()
            }}
          />
          <Select
            aria-label={t("keywordHighlight.kind")}
            value={rule.kind}
            className="w-32"
            onChange={(event) =>
              onChange({ kind: event.target.value === "regex" ? "regex" : "keyword" })
            }
          >
            <option value="keyword">{t("keywordHighlight.kindKeyword")}</option>
            <option value="regex">{t("keywordHighlight.kindRegex")}</option>
          </Select>
        </div>
        {error && error !== "empty" && (
          <p className="text-destructive text-xs">{t(`keywordHighlight.errors.${error}`)}</p>
        )}
      </div>

      <div className="flex flex-wrap gap-x-6 gap-y-2">
        <Label className="flex items-center gap-2 font-normal">
          <Checkbox
            checked={rule.case_sensitive}
            onCheckedChange={(checked) => onChange({ case_sensitive: checked })}
          />
          {t("keywordHighlight.caseSensitive")}
        </Label>
        <Label className="flex items-center gap-2 font-normal">
          <Checkbox
            checked={rule.whole_word}
            onCheckedChange={(checked) => onChange({ whole_word: checked })}
          />
          {t("keywordHighlight.wholeWord")}
        </Label>
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <div className="space-y-1.5">
          <div className="text-sm font-medium">{t("keywordHighlight.foreground")}</div>
          <HighlightColorPicker
            label={t("keywordHighlight.foreground")}
            value={rule.foreground}
            palette={palette}
            onChange={(foreground) => onChange({ foreground })}
          />
        </div>
        <div className="space-y-1.5">
          <div className="text-sm font-medium">{t("keywordHighlight.background")}</div>
          <HighlightColorPicker
            label={t("keywordHighlight.background")}
            value={rule.background}
            palette={palette}
            onChange={(background) => onChange({ background })}
          />
        </div>
      </div>
    </div>
  )
}
