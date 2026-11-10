import { useTranslation } from "react-i18next"

import type { AgentResumeNotice } from "@/components/TerminalTab/agentResume"
import { Button } from "@/components/ui/button"

/** A resume request never types anything until this button is clicked. */
export function AgentResumeBar({ notice }: { notice: AgentResumeNotice }) {
  const { t } = useTranslation()
  return (
    <div
      role="status"
      className="bg-card absolute top-3 left-3 z-20 flex max-w-lg items-center gap-3 rounded-md border p-3 text-xs shadow-lg"
    >
      <span>{t(`agentResume.${notice.reason}`)}</span>
      <Button size="xs" disabled={!notice.enabled} onClick={notice.resume}>
        {t("agentResume.continue")}
      </Button>
    </div>
  )
}
