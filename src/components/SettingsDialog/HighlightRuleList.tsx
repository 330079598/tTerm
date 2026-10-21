import React from "react"
import { ArrowDown, ArrowUp, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { TerminalColors } from "@/components/SettingsDialog/HighlightPreview"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import type { HighlightRule } from "@/lib/keywordHighlight/config"
import { highlightPatternError, resolveHighlightColor } from "@/lib/keywordHighlight/match"
import { cn } from "@/lib/utils"

interface HighlightRuleListProps {
  rules: readonly HighlightRule[]
  selectedId: string | null
  colors: TerminalColors
  onSelect: (ruleId: string) => void
  onToggle: (ruleId: string, enabled: boolean) => void
  onMove: (ruleId: string, offset: -1 | 1) => void
  onRemove: (ruleId: string) => void
}

/** A set's rules in order, each showing its pattern in its colors. */
export const HighlightRuleList: React.FC<HighlightRuleListProps> = ({
  rules,
  selectedId,
  colors,
  onSelect,
  onToggle,
  onMove,
  onRemove,
}) => {
  const { t } = useTranslation()

  if (rules.length === 0) {
    return (
      <p className="text-muted-foreground rounded-md border border-dashed px-3 py-4 text-center text-xs">
        {t("keywordHighlight.noRules")}
      </p>
    )
  }

  return (
    <ul
      className="divide-border overflow-hidden rounded-md border"
      aria-label={t("keywordHighlight.rules")}
    >
      {rules.map((rule, index) => {
        const selected = rule.id === selectedId
        const invalid = highlightPatternError(rule) !== null
        return (
          <li
            key={rule.id}
            className={cn(
              "flex items-center gap-2 border-b px-2 py-1.5 last:border-b-0",
              selected ? "bg-accent" : "hover:bg-accent/50"
            )}
          >
            <Checkbox
              checked={rule.enabled}
              aria-label={t("keywordHighlight.ruleEnabled")}
              onCheckedChange={(checked) => onToggle(rule.id, checked)}
            />
            <button
              type="button"
              aria-pressed={selected}
              className="flex min-w-0 flex-1 cursor-pointer items-center gap-2 text-left"
              onClick={() => onSelect(rule.id)}
            >
              <span
                className={cn(
                  "min-w-0 truncate rounded px-1.5 py-0.5 font-mono text-xs",
                  !rule.enabled && "opacity-50"
                )}
                style={{
                  backgroundColor:
                    resolveHighlightColor(rule.background, colors) ?? colors.background,
                  color: resolveHighlightColor(rule.foreground, colors) ?? colors.foreground,
                }}
              >
                {rule.pattern || t("keywordHighlight.emptyPattern")}
              </span>
              <span className="text-muted-foreground shrink-0 text-[11px]">
                {t(
                  rule.kind === "regex"
                    ? "keywordHighlight.kindRegex"
                    : "keywordHighlight.kindKeyword"
                )}
              </span>
              {invalid && (
                <span className="text-destructive shrink-0 text-[11px]">
                  {t("keywordHighlight.ruleInactive")}
                </span>
              )}
            </button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={t("keywordHighlight.moveUp")}
              title={t("keywordHighlight.moveUp")}
              disabled={index === 0}
              onClick={() => onMove(rule.id, -1)}
            >
              <ArrowUp />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={t("keywordHighlight.moveDown")}
              title={t("keywordHighlight.moveDown")}
              disabled={index === rules.length - 1}
              onClick={() => onMove(rule.id, 1)}
            >
              <ArrowDown />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={t("keywordHighlight.deleteRule")}
              title={t("keywordHighlight.deleteRule")}
              onClick={() => onRemove(rule.id)}
            >
              <Trash2 />
            </Button>
          </li>
        )
      })}
    </ul>
  )
}
