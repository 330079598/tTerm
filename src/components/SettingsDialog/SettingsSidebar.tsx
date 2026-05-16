import React from "react"
import { Info, Palette, Shield, Type, UploadCloud } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TabsList, TabsTrigger } from "@/components/ui/tabs"

export const SettingsSidebar: React.FC = () => {
  const { t } = useTranslation()

  return (
    <div className="border-border bg-muted/30 w-48 border-r p-3">
      <TabsList className="flex h-auto w-full flex-col gap-1 bg-transparent">
        <TabsTrigger
          value="appearance"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <Palette size={16} />
          {t("settings.appearance")}
        </TabsTrigger>
        <TabsTrigger
          value="font"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <Type size={16} />
          {t("settings.font")}
        </TabsTrigger>
        <TabsTrigger
          value="security"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <Shield size={16} />
          {t("settings.security")}
        </TabsTrigger>
        <TabsTrigger
          value="general"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <Info size={16} />
          {t("settings.general")}
        </TabsTrigger>
        <TabsTrigger
          value="updates"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <UploadCloud size={16} />
          {t("settings.updates", { defaultValue: "Updates" })}
        </TabsTrigger>
      </TabsList>
    </div>
  )
}
