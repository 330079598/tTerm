import React from "react"
import { Bot } from "lucide-react"
import { useTranslation } from "react-i18next"

import { SettingsRow, SettingsSection } from "@/components/SettingsDialog/SettingsLayout"
import { TabAgentBadge } from "@/components/TabAttentionDot"
import { ColorSwatches } from "@/components/ui/color-swatches"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { useSyncedState } from "@/hooks/useSyncedState"
import type { AgentStatus } from "@/lib/agentStatus"
import {
  TAB_AGENT_WAITING_STYLES,
  TAB_AGENT_WORKING_STYLES,
  type TabAgentIndicatorConfig,
  type TabAgentWaitingStyle,
  type TabAgentWorkingStyle,
} from "@/lib/tabAgentIndicator"
import { TAB_ICON_COLORS } from "@/lib/tabIcons"
import { cn } from "@/lib/utils"

// A custom color reports every step while its picker is dragged; saving waits
// for it to settle.
const SAVE_DELAY_MS = 400

/** Settings for the badge a tab shows while an AI agent works in it or waits for the user. */
export function TabAgentIndicatorSettings() {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [indicator, setIndicator] = useSyncedState(config.tab_agent_indicator)
  const pendingRef = React.useRef<{ timer: number; next: TabAgentIndicatorConfig } | null>(null)

  const flush = React.useCallback(() => {
    const pending = pendingRef.current
    if (!pending) return
    window.clearTimeout(pending.timer)
    pendingRef.current = null
    void saveSettings({ tab_agent_indicator: pending.next })
  }, [saveSettings])

  React.useEffect(() => flush, [flush])

  const save = (patch: Partial<TabAgentIndicatorConfig>) => {
    const next = { ...indicator, ...patch }
    setIndicator(next)
    if (pendingRef.current) window.clearTimeout(pendingRef.current.timer)
    pendingRef.current = { timer: window.setTimeout(flush, SAVE_DELAY_MS), next }
  }

  return (
    <SettingsSection
      icon={<Bot size={16} />}
      title={t("tabAgentIndicator.title")}
      description={t("tabAgentIndicator.description")}
    >
      <SettingsRow
        title={t("tabAgentIndicator.working")}
        description={t("tabAgentIndicator.workingDescription")}
      >
        <AgentStyleChoices
          activity="processing"
          styles={TAB_AGENT_WORKING_STYLES}
          style={indicator.working}
          color={indicator.working_color}
          onStyleChange={(working) => save({ working })}
          onColorChange={(working_color) => save({ working_color })}
        />
      </SettingsRow>
      <SettingsRow
        title={t("tabAgentIndicator.waiting")}
        description={t("tabAgentIndicator.waitingDescription")}
      >
        <AgentStyleChoices
          activity="waiting"
          styles={TAB_AGENT_WAITING_STYLES}
          style={indicator.waiting}
          color={indicator.waiting_color}
          onStyleChange={(waiting) => save({ waiting })}
          onColorChange={(waiting_color) => save({ waiting_color })}
        />
      </SettingsRow>
    </SettingsSection>
  )
}

interface AgentStyleChoicesProps<S extends TabAgentWorkingStyle | TabAgentWaitingStyle> {
  activity: AgentStatus["activity"]
  styles: readonly S[]
  style: S
  color: string
  onStyleChange: (style: S) => void
  onColorChange: (color: string) => void
}

/** The styles for one activity, each shown as it runs on a tab, and its color. */
function AgentStyleChoices<S extends TabAgentWorkingStyle | TabAgentWaitingStyle>({
  activity,
  styles,
  style,
  color,
  onStyleChange,
  onColorChange,
}: AgentStyleChoicesProps<S>) {
  const { t } = useTranslation()
  const group = activity === "processing" ? "workingStyles" : "waitingStyles"
  const themeColor = activity === "processing" ? "--primary" : "--warning"

  return (
    <div className="space-y-3">
      <div
        className="flex flex-wrap gap-1.5"
        role="radiogroup"
        aria-label={t("tabAgentIndicator.style")}
      >
        {styles.map((choice) => (
          <button
            key={choice}
            type="button"
            role="radio"
            aria-checked={style === choice}
            className={cn(
              "hover:bg-muted flex h-8 items-center gap-2 rounded-md border px-2.5 text-xs",
              style === choice ? "border-primary/50 bg-primary/10" : "border-border"
            )}
            onClick={() => onStyleChange(choice)}
          >
            <span className="flex h-3 items-center" aria-hidden="true">
              {/* Keyed by color too, so a waiting badge replays its ripple on a change. */}
              <TabAgentBadge
                key={color}
                activity={activity}
                style={choice}
                color={color}
                label=""
              />
            </span>
            {t(`tabAgentIndicator.${group}.${choice}`)}
          </button>
        ))}
      </div>
      {/* The crab keeps its own colors, so there is no color to pick for it. */}
      {style === "crab" ? (
        <p className="text-muted-foreground text-xs">{t("tabAgentIndicator.crabColors")}</p>
      ) : (
        <div className="space-y-1.5">
          <div className="text-muted-foreground text-xs">{t("tabAgentIndicator.color")}</div>
          <ColorSwatches
            value={color}
            colors={TAB_ICON_COLORS}
            onChange={onColorChange}
            label={t("tabAgentIndicator.color")}
            empty={{
              label: t("tabAgentIndicator.themeColor"),
              content: (
                <span
                  className="h-4 w-4 rounded-full"
                  style={{ backgroundColor: `hsl(var(${themeColor}))` }}
                />
              ),
            }}
            customLabel={t("tabAgentIndicator.customColor")}
          />
        </div>
      )}
    </div>
  )
}
