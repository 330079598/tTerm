import React from "react"
import {
  FileClock,
  HardDriveDownload,
  FolderTree,
  Info,
  Palette,
  Plug,
  Shield,
  SquareTerminal,
  UploadCloud,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { TabsList, TabsTrigger } from "@/components/ui/tabs"

export const SettingsSidebar: React.FC = () => {
  const { t } = useTranslation()

  return (
    <div className="border-border bg-muted/30 w-48 border-r p-3">
      <TabsList className="flex h-auto w-full flex-col gap-1 bg-transparent">
        <TabsTrigger
          value="profile-groups"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <FolderTree size={16} />
          {t("settings.profileGroups", { defaultValue: "Groups" })}
        </TabsTrigger>
        <TabsTrigger
          value="appearance"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <Palette size={16} />
          {t("settings.appearance")}
        </TabsTrigger>
        <TabsTrigger
          value="terminal"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <SquareTerminal size={16} />
          {t("settings.terminal", { defaultValue: "Terminal" })}
        </TabsTrigger>
        <TabsTrigger
          value="connection"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <Plug size={16} />
          {t("settings.connection", { defaultValue: "Connection" })}
        </TabsTrigger>
        <TabsTrigger
          value="logging"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <FileClock size={16} />
          {t("terminalLogging.title")}
        </TabsTrigger>
        <TabsTrigger
          value="data-migration"
          className="data-[state=active]:bg-background w-full justify-start gap-2 data-[state=active]:shadow-sm"
        >
          <HardDriveDownload size={16} />
          {t("dataMigration.sidebar")}
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
