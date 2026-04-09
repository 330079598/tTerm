import React from "react"
import { Info, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"

interface GeneralSettingsTabProps {
  handleAbout: () => void
  handleClearSession: () => Promise<void>
}

export const GeneralSettingsTab: React.FC<GeneralSettingsTabProps> = ({
  handleAbout,
  handleClearSession,
}) => {
  const { t } = useTranslation()

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-3">
        <Card className="overflow-hidden border-transparent shadow-none">
          <CardContent className="p-0">
            <Button
              type="button"
              variant="ghost"
              onClick={handleClearSession}
              className="h-auto w-full justify-start gap-3 rounded-lg px-4 py-3 text-left"
            >
              <Trash2 size={16} className="text-destructive" />
              <div>
                <div className="text-sm font-medium">{t("settings.clearSession")}</div>
                <div className="text-muted-foreground text-xs">
                  {t("settings.clearSessionDesc")}
                </div>
              </div>
            </Button>
          </CardContent>
        </Card>

        <Card className="overflow-hidden border-transparent shadow-none">
          <CardContent className="p-0">
            <Button
              type="button"
              variant="ghost"
              onClick={handleAbout}
              className="h-auto w-full justify-start gap-3 rounded-lg px-4 py-3 text-left"
            >
              <Info size={16} />
              <div>
                <div className="text-sm font-medium">{t("settings.about")}</div>
                <div className="text-muted-foreground text-xs">{t("app.subtitle")}</div>
              </div>
            </Button>
          </CardContent>
        </Card>
      </div>
    </ScrollArea>
  )
}
