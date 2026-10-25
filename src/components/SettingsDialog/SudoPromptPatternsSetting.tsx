import React from "react"
import { KeyRound } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow } from "@/components/SettingsDialog/SettingsLayout"
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
 * Extra password prompts to recognize, one regular expression per line. The
 * draft is saved when the field loses focus, and only once every line is a
 * valid pattern.
 */
export const SudoPromptPatternsSetting: React.FC<SudoPromptPatternsSettingProps> = ({
  onChange,
  patterns,
}) => {
  const { t } = useTranslation()
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
      description={t("settings.sudoPromptPatternsDesc")}
    >
      <textarea
        aria-invalid={invalidPatterns.length > 0}
        aria-label={t("settings.sudoPromptPatterns")}
        autoCapitalize="none"
        autoCorrect="off"
        className={cn(
          "border-input placeholder:text-muted-foreground dark:bg-input/30 min-h-20 w-full rounded-md border bg-transparent px-3 py-2 font-mono text-xs shadow-xs outline-none",
          "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
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
    </SettingsRow>
  )
}
