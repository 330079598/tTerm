import React, { useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { Textarea } from "@/components/ui/textarea"
import type { HighlightSet } from "@/lib/keywordHighlight/config"
import {
  compileHighlightSet,
  type HighlightPalette,
  type HighlightSpan,
  matchHighlights,
} from "@/lib/keywordHighlight/match"

/** The terminal's colors: its palette and the text and background around it. */
export type TerminalColors = HighlightPalette & { background?: string; foreground?: string }

/** Text cut into plain runs and highlighted ones. */
function pieces(text: string, spans: readonly HighlightSpan[]) {
  const result: { text: string; span?: HighlightSpan }[] = []
  let position = 0
  for (const span of spans) {
    if (span.start > position) result.push({ text: text.slice(position, span.start) })
    result.push({ text: text.slice(span.start, span.end), span })
    position = span.end
  }
  if (position < text.length) result.push({ text: text.slice(position) })
  return result
}

interface HighlightPreviewProps {
  set: HighlightSet
  colors: TerminalColors
  fontFamily: string
}

/** Sample output, which the user may replace with their own, as the set colors it. */
export const HighlightPreview: React.FC<HighlightPreviewProps> = ({ set, colors, fontFamily }) => {
  const { t } = useTranslation()
  const [sample, setSample] = useState(() => t("keywordHighlight.previewSample"))
  const lines = useMemo(() => {
    const rules = compileHighlightSet(set, colors)
    return sample.split("\n").map((line) => pieces(line, matchHighlights(line, rules)))
  }, [colors, sample, set])

  return (
    <div className="space-y-2">
      <pre
        aria-label={t("keywordHighlight.preview")}
        className="overflow-x-auto rounded-md border px-3 py-2 text-xs leading-5"
        style={{ background: colors.background, color: colors.foreground, fontFamily }}
      >
        {lines.map((line, index) => (
          <div key={index}>
            {line.length === 0
              ? " "
              : line.map((piece, pieceIndex) => (
                  <span
                    key={pieceIndex}
                    style={
                      piece.span
                        ? { color: piece.span.foreground, background: piece.span.background }
                        : undefined
                    }
                  >
                    {piece.text}
                  </span>
                ))}
          </div>
        ))}
      </pre>
      <Textarea
        aria-label={t("keywordHighlight.previewEdit")}
        value={sample}
        rows={3}
        spellCheck={false}
        className="font-mono text-xs"
        onChange={(event) => setSample(event.target.value)}
      />
    </div>
  )
}
