import React from "react"
import { AArrowDown, AArrowUp } from "lucide-react"
import { useTranslation } from "react-i18next"

import { stepReplayFontSize } from "@/components/LogsPanel/replayView"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { MAX_REPLAY_FONT_SIZE, MIN_REPLAY_FONT_SIZE } from "@/lib/terminalLogReplay"

interface ReplayFontControlsProps {
  /** The size the replay is drawn at. */
  fontSize: number
  /** The panel keeps the font from growing any further. */
  capped: boolean
  fitPanel: boolean
  onFontSizeChange: (fontSize: number) => void
  onFitPanelChange: (fitPanel: boolean) => void
}

/** Sets the replay's font size, or lets it fit the panel. */
export const ReplayFontControls: React.FC<ReplayFontControlsProps> = ({
  fontSize,
  capped,
  fitPanel,
  onFontSizeChange,
  onFitPanelChange,
}) => {
  const { t } = useTranslation()
  const shown = Math.round(fontSize * 4) / 4

  return (
    <>
      <div
        className="flex items-center gap-0.5"
        role="group"
        aria-label={t("terminalLogs.fontSize")}
      >
        <Button
          type="button"
          size="icon"
          variant="ghost"
          onClick={() => onFontSizeChange(stepReplayFontSize(fontSize, -1))}
          disabled={fitPanel || fontSize <= MIN_REPLAY_FONT_SIZE}
          aria-label={t("terminalLogs.fontSmaller")}
          title={t("terminalLogs.fontSmaller")}
        >
          <AArrowDown />
        </Button>
        <span
          className="text-muted-foreground w-9 text-center font-mono text-xs tabular-nums"
          title={t("terminalLogs.fontSize")}
        >
          {shown}
        </span>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          onClick={() => onFontSizeChange(stepReplayFontSize(fontSize, 1))}
          disabled={fitPanel || capped || fontSize >= MAX_REPLAY_FONT_SIZE}
          aria-label={t("terminalLogs.fontLarger")}
          title={t("terminalLogs.fontLarger")}
        >
          <AArrowUp />
        </Button>
      </div>
      <div className="flex items-center gap-1.5">
        <Checkbox id="replay-fit-panel" checked={fitPanel} onCheckedChange={onFitPanelChange} />
        <Label
          htmlFor="replay-fit-panel"
          className="text-xs font-normal"
          title={t("terminalLogs.fitPanelDesc")}
        >
          {t("terminalLogs.fitPanel")}
        </Label>
      </div>
    </>
  )
}
