import type { Dispatch, SetStateAction } from "react"
import { Plus, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { addCommandTags, removeCommandTag } from "@/components/CommandLibrary/commandLibraryUtils"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { isImeKeyEvent } from "@/lib/ime"

interface CommandTagsFieldProps {
  selectedTags: string[]
  setSelectedTags: Dispatch<SetStateAction<string[]>>
  tagDraft: string
  setTagDraft: (tagDraft: string) => void
  availableTags: string[]
}

/** The command's tags: the chosen ones, a field to add more, and the ones in use elsewhere. */
export function CommandTagsField({
  selectedTags,
  setSelectedTags,
  tagDraft,
  setTagDraft,
  availableTags,
}: CommandTagsFieldProps) {
  const { t } = useTranslation()

  const remainingTags = availableTags.filter(
    (availableTag) =>
      !selectedTags.some(
        (selectedTag) => selectedTag.toLocaleLowerCase() === availableTag.toLocaleLowerCase()
      )
  )

  const commitTagDraft = () => {
    if (!tagDraft.trim()) return
    setSelectedTags((current) => addCommandTags(current, tagDraft))
    setTagDraft("")
  }

  return (
    <div className="space-y-2">
      <Label htmlFor="saved-command-tags">{t("commandLibrary.form.tags")}</Label>
      {selectedTags.length > 0 && (
        <div
          className="border-input bg-muted/20 flex min-h-11 flex-wrap items-center gap-2 rounded-md border px-2 py-2"
          aria-label={t("commandLibrary.form.selectedTags")}
          role="list"
        >
          {selectedTags.map((tag) => (
            <span
              key={tag.toLocaleLowerCase()}
              className="bg-secondary text-secondary-foreground inline-flex min-h-7 max-w-full items-center gap-1 rounded-md border px-2 text-xs font-medium"
              role="listitem"
            >
              <span className="truncate">{tag}</span>
              <button
                type="button"
                className="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/50 -mr-1 flex size-6 shrink-0 items-center justify-center rounded-sm transition-colors outline-none focus-visible:ring-[3px]"
                aria-label={t("commandLibrary.form.removeTag", { tag })}
                onClick={() => setSelectedTags((current) => removeCommandTag(current, tag))}
              >
                <X className="size-3.5" />
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="flex items-stretch gap-2">
        <Input
          id="saved-command-tags"
          value={tagDraft}
          maxLength={64}
          className="h-11"
          placeholder={t("commandLibrary.form.tagsPlaceholder")}
          aria-describedby="saved-command-tags-hint"
          onKeyDown={(event) => {
            if (isImeKeyEvent(event.nativeEvent)) return
            if (event.key !== "Enter" && event.key !== "," && event.key !== "，") return
            event.preventDefault()
            commitTagDraft()
          }}
          onChange={(event) => {
            const value = event.target.value
            if (/[,，\n]/.test(value)) {
              setSelectedTags((current) => addCommandTags(current, value))
              setTagDraft("")
              return
            }
            setTagDraft(value)
          }}
        />
        <Button
          type="button"
          variant="outline"
          className="h-11 shrink-0"
          disabled={!tagDraft.trim()}
          onClick={commitTagDraft}
        >
          <Plus />
          {t("commandLibrary.form.addTag")}
        </Button>
      </div>
      {remainingTags.length > 0 && (
        <div className="space-y-2 pt-1">
          <p className="text-muted-foreground text-xs font-medium">
            {t("commandLibrary.form.availableTags")}
          </p>
          <div className="flex max-h-28 flex-wrap gap-2 overflow-y-auto pr-1">
            {remainingTags.map((tag) => (
              <button
                key={tag.toLocaleLowerCase()}
                type="button"
                className="border-input bg-background text-muted-foreground hover:bg-accent hover:text-accent-foreground focus-visible:ring-ring/50 inline-flex min-h-8 max-w-full items-center gap-1.5 rounded-md border px-2.5 text-xs font-medium transition-colors outline-none focus-visible:ring-[3px]"
                onClick={() => setSelectedTags((current) => addCommandTags(current, tag))}
              >
                <Plus className="size-3.5 shrink-0" />
                <span className="truncate">{tag}</span>
              </button>
            ))}
          </div>
        </div>
      )}
      <p id="saved-command-tags-hint" className="text-muted-foreground text-xs">
        {t("commandLibrary.form.tagsHint")}
      </p>
    </div>
  )
}
