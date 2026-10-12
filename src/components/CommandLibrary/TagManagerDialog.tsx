import { invoke } from "@tauri-apps/api/core"
import { Check, Loader2, Pencil, Plus, Trash2, X } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "react-i18next"

import type { CommandLibraryData } from "@/components/CommandLibrary/useCommandLibraryData"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { useToast } from "@/hooks/use-toast"
import { isImeKeyEvent } from "@/lib/ime"

type TagManagerDialogProps = Pick<CommandLibraryData, "tags" | "setTags" | "loadCommands"> & {
  tagManagerOpen: boolean
  setTagManagerOpen: (open: boolean) => void
}

/** Adding, renaming and deleting the tags commands can have. */
export function TagManagerDialog({
  tagManagerOpen,
  setTagManagerOpen,
  tags,
  setTags,
  loadCommands,
}: TagManagerDialogProps) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [managedTagDraft, setManagedTagDraft] = useState("")
  const [editingTag, setEditingTag] = useState<string | null>(null)
  const [tagMutationPending, setTagMutationPending] = useState(false)

  const saveManagedTag = async () => {
    const value = managedTagDraft.trim()
    if (!value || tagMutationPending) return
    setTagMutationPending(true)
    try {
      if (editingTag) {
        setTags(await invoke<string[]>("rename_command_tag", { oldTag: editingTag, newTag: value }))
        await loadCommands()
      } else {
        await invoke<string>("create_command_tag", { tag: value })
        setTags(await invoke<string[]>("list_command_tags"))
      }
      setManagedTagDraft("")
      setEditingTag(null)
    } catch (error) {
      toast({
        title: t("commandLibrary.tags.saveFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      setTagMutationPending(false)
    }
  }

  const deleteManagedTag = async (tag: string) => {
    if (tagMutationPending) return
    setTagMutationPending(true)
    try {
      setTags(await invoke<string[]>("delete_command_tag", { tag }))
      await loadCommands()
    } catch (error) {
      toast({
        title: t("commandLibrary.tags.deleteFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      setTagMutationPending(false)
    }
  }

  return (
    <Dialog open={tagManagerOpen} onOpenChange={setTagManagerOpen}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("commandLibrary.tags.title")}</DialogTitle>
          <DialogDescription>{t("commandLibrary.tags.description")}</DialogDescription>
        </DialogHeader>
        <div className="flex min-w-0 gap-2">
          <Input
            value={managedTagDraft}
            maxLength={64}
            placeholder={
              editingTag
                ? t("commandLibrary.tags.renamePlaceholder")
                : t("commandLibrary.tags.addPlaceholder")
            }
            aria-label={t("commandLibrary.tags.name")}
            onChange={(event) => setManagedTagDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
                event.preventDefault()
                void saveManagedTag()
              }
            }}
          />
          <Button
            type="button"
            className="shrink-0"
            disabled={!managedTagDraft.trim() || tagMutationPending}
            onClick={() => void saveManagedTag()}
          >
            {tagMutationPending ? (
              <Loader2 className="animate-spin" />
            ) : editingTag ? (
              <Check />
            ) : (
              <Plus />
            )}
            {editingTag ? t("common.save") : t("commandLibrary.tags.add")}
          </Button>
        </div>
        {editingTag && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="w-fit"
            onClick={() => {
              setEditingTag(null)
              setManagedTagDraft("")
            }}
          >
            <X />
            {t("common.cancel")}
          </Button>
        )}
        <ScrollArea className="max-h-72">
          {tags.length === 0 ? (
            <p className="text-muted-foreground py-8 text-center text-sm">
              {t("commandLibrary.tags.empty")}
            </p>
          ) : (
            <div className="divide-y rounded-md border">
              {tags.map((tag) => (
                <div
                  key={tag.toLocaleLowerCase()}
                  className="hover:bg-muted/35 flex min-h-12 items-center gap-2 px-3 transition-colors"
                >
                  <span className="min-w-0 flex-1 truncate text-sm">{tag}</span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t("commandLibrary.tags.rename", { tag })}
                    onClick={() => {
                      setEditingTag(tag)
                      setManagedTagDraft(tag)
                    }}
                  >
                    <Pencil />
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="hover:text-destructive"
                    aria-label={t("commandLibrary.tags.delete", { tag })}
                    onClick={() => void deleteManagedTag(tag)}
                  >
                    <Trash2 />
                  </Button>
                </div>
              ))}
            </div>
          )}
        </ScrollArea>
      </DialogContent>
    </Dialog>
  )
}
