import React, { useId, useState } from "react"
import { ChevronDown, ChevronUp, KeyRound } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow } from "@/components/SettingsDialog/SettingsLayout"
import { Button } from "@/components/ui/button"
import { useSyncedState } from "@/hooks/useSyncedState"
import { isValidPromptPattern } from "@/lib/sudoPrompt"
import { cn } from "@/lib/utils"

interface SudoPromptPatternsSettingProps {
  onChange: (patterns: string[]) => Promise<void>
  patterns: string[]
}

function parsePromptPatterns(draft: string): string[] {
  return draft
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
}

/**
 * Extra password prompts to recognize, one regular expression per line. Few
 * people need it, so it starts folded away behind a summary of what is
 * recognized already. The draft is saved when the field loses focus, and only
 * once every line is a valid pattern.
 */
export const SudoPromptPatternsSetting: React.FC<SudoPromptPatternsSettingProps> = ({
  onChange,
  patterns,
}) => {
  const { t } = useTranslation()
  const fieldId = useId()
  const [expanded, setExpanded] = useState(false)
  const [draft, setDraft] = useSyncedState(patterns.join("\n"))
  const invalidPatterns = parsePromptPatterns(draft).filter(
    (pattern) => !isValidPromptPattern(pattern)
  )

  const commit = React.useCallback(() => {
    const next = parsePromptPatterns(draft)
    if (next.some((pattern) => !isValidPromptPattern(pattern))) return
    if (next.join("\n") !== patterns.join("\n")) {
      void onChange(next)
    }
  }, [onChange, patterns, draft])

  return (
    <SettingsRow
      icon={<KeyRound size={16} />}
      title={t("settings.sudoPromptPatterns")}
      description={
        patterns.length > 0
          ? t("settings.sudoPromptPatternsSummaryCount", { count: patterns.length })
          : t("settings.sudoPromptPatternsSummary")
      }
      action={
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-controls={fieldId}
          aria-expanded={expanded}
          onClick={() => setExpanded((open) => !open)}
        >
          {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          {expanded ? t("settings.sudoPromptPatternsHide") : t("settings.sudoPromptPatternsShow")}
        </Button>
      }
    >
      {expanded && (
        <div id={fieldId}>
          <p className="text-muted-foreground mb-2 text-xs leading-5">
            {t("settings.sudoPromptPatternsDesc")}
          </p>
          <textarea
            aria-invalid={invalidPatterns.length > 0}
            aria-label={t("settings.sudoPromptPatternsField")}
            autoCapitalize="none"
            autoCorrect="off"
            className={cn(
              "border-input placeholder:text-muted-foreground dark:bg-input/30 min-h-20 w-full rounded-md border bg-transparent px-3 py-2 font-mono text-xs shadow-xs outline-none",
              "focus-visible:border-ring",
              "aria-invalid:border-destructive"
            )}
            onBlur={commit}
            onChange={(event) => setDraft(event.target.value)}
            placeholder={"^Password for (?<user>\\S+) on \\S+:$"}
            spellCheck={false}
            value={draft}
          />
          {invalidPatterns.length > 0 && (
            <p className="text-destructive mt-1 text-xs">
              {t("settings.sudoPromptPatternsInvalid", { pattern: invalidPatterns[0] })}
            </p>
          )}
        </div>
      )}
    </SettingsRow>
  )
}
