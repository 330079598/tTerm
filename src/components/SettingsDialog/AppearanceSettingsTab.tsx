import React from "react"
import { Check, Copy, Edit, Languages, Palette, Plus, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { ScrollArea } from "@/components/ui/scroll-area"
import { cn } from "@/lib/utils"
import type { CustomTheme, PresetTheme } from "@/types/theme"

interface LanguageOption {
  code: string
  label: string
  nativeLabel: string
}

interface AppearanceSettingsTabProps {
  currentTheme: string
  customThemes: CustomTheme[]
  getThemeColor: (themeId: string) => string
  handleDeleteTheme: (themeId: string) => Promise<void>
  handleDuplicateTheme: (themeId: string) => Promise<void>
  handleLanguageChange: (langCode: string) => Promise<void>
  handleThemeChange: (themeId: string) => Promise<void>
  i18nLanguage: string
  languages: LanguageOption[]
  presetThemes: PresetTheme[]
  setCreatingFromTheme: React.Dispatch<React.SetStateAction<string | null>>
  setEditingThemeId: React.Dispatch<React.SetStateAction<string | null>>
}

export const AppearanceSettingsTab: React.FC<AppearanceSettingsTabProps> = ({
  currentTheme,
  customThemes,
  getThemeColor,
  handleDeleteTheme,
  handleDuplicateTheme,
  handleLanguageChange,
  handleThemeChange,
  i18nLanguage,
  languages,
  presetThemes,
  setCreatingFromTheme,
  setEditingThemeId,
}) => {
  const { t } = useTranslation()

  return (
    <ScrollArea className="h-full pr-4">
      <div className="space-y-6">
        <div>
          <div className="mb-3 flex items-center gap-2">
            <Languages size={16} />
            <h3 className="text-sm font-semibold">{t("language.title")}</h3>
          </div>
          <div className="grid gap-2">
            {languages.map((lang) => {
              const isActive = i18nLanguage === lang.code
              return (
                <Card key={lang.code} className="overflow-hidden border-transparent shadow-none">
                  <CardContent className="p-0">
                    <Button
                      type="button"
                      variant="ghost"
                      onClick={() => handleLanguageChange(lang.code)}
                      className={cn(
                        "h-auto w-full justify-between rounded-lg px-4 py-3 text-left",
                        isActive && "bg-muted"
                      )}
                    >
                      <div>
                        <div className="text-sm font-semibold">{lang.nativeLabel}</div>
                        <div className="text-muted-foreground text-xs">{lang.label}</div>
                      </div>
                      {isActive && <Check size={16} className="text-primary" />}
                    </Button>
                  </CardContent>
                </Card>
              )
            })}
          </div>
        </div>

        <div>
          <div className="mb-3 flex items-center gap-2">
            <Palette size={16} />
            <h3 className="text-sm font-semibold">{t("theme.title")}</h3>
          </div>

          <div className="mb-4">
            <h4 className="text-muted-foreground mb-2 text-xs font-medium">
              {t("themeEditor.presetThemes")}
            </h4>
            <div className="grid gap-2">
              {presetThemes.map((theme) => (
                <Card key={theme.id} className="overflow-hidden border-transparent shadow-none">
                  <CardContent className="p-0">
                    <div className="flex items-center">
                      <Button
                        type="button"
                        variant="ghost"
                        onClick={() => handleThemeChange(theme.id)}
                        className={cn(
                          "h-auto flex-1 justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
                          currentTheme === theme.id
                            ? "border-primary bg-accent"
                            : "border-transparent"
                        )}
                      >
                        <span
                          className="border-border size-5 shrink-0 rounded-full border"
                          style={{ background: getThemeColor(theme.id) }}
                        />
                        <div className="flex flex-col items-start">
                          <span className="text-sm leading-none font-medium">
                            {t(`theme.${theme.id}`)}
                          </span>
                          <span className="text-muted-foreground mt-1 text-xs">
                            {t(`theme.${theme.id}Desc`)}
                          </span>
                        </div>
                        {currentTheme === theme.id && (
                          <Check size={16} className="text-primary ml-auto" />
                        )}
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={() => setCreatingFromTheme(theme.id)}
                        className="h-auto px-2 py-2"
                        title={t("themeEditor.customize")}
                      >
                        <Plus size={16} />
                      </Button>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>

          {customThemes.length > 0 && (
            <div>
              <h4 className="text-muted-foreground mb-2 text-xs font-medium">
                {t("themeEditor.customThemes")}
              </h4>
              <div className="grid gap-2">
                {customThemes.map((theme) => (
                  <Card key={theme.id} className="overflow-hidden border-transparent shadow-none">
                    <CardContent className="p-0">
                      <div className="flex items-center">
                        <Button
                          type="button"
                          variant="ghost"
                          onClick={() => handleThemeChange(theme.id)}
                          className={cn(
                            "h-auto flex-1 justify-start gap-3 rounded-lg border px-3 py-2.5 text-left",
                            currentTheme === theme.id
                              ? "border-primary bg-accent"
                              : "border-transparent"
                          )}
                        >
                          <span
                            className="border-border size-5 shrink-0 rounded-full border"
                            style={{ background: getThemeColor(theme.id) }}
                          />
                          <div className="flex flex-col items-start">
                            <span className="text-sm leading-none font-medium">{theme.name}</span>
                            {theme.description && (
                              <span className="text-muted-foreground mt-1 text-xs">
                                {theme.description}
                              </span>
                            )}
                          </div>
                          {currentTheme === theme.id && (
                            <Check size={16} className="text-primary ml-auto" />
                          )}
                        </Button>
                        <div className="flex">
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => setEditingThemeId(theme.id)}
                            className="h-auto px-2 py-2"
                            title={t("themeEditor.edit")}
                          >
                            <Edit size={14} />
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => handleDuplicateTheme(theme.id)}
                            className="h-auto px-2 py-2"
                            title={t("themeEditor.duplicate")}
                          >
                            <Copy size={14} />
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => handleDeleteTheme(theme.id)}
                            className="text-destructive hover:text-destructive h-auto px-2 py-2"
                            title={t("themeEditor.delete")}
                          >
                            <Trash2 size={14} />
                          </Button>
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            </div>
          )}

          <Button
            variant="outline"
            className="mt-3 w-full"
            onClick={() => setCreatingFromTheme("default")}
          >
            <Plus size={16} className="mr-2" />
            {t("themeEditor.createNew")}
          </Button>
        </div>
      </div>
    </ScrollArea>
  )
}
