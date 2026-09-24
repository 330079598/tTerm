import { toast } from "@/hooks/use-toast"

/** The backend declined the write because the prompt was no longer waiting. */
export function notifySavedPasswordNotSent(t: (key: string) => string) {
  toast({
    title: t("sudoAutofill.notSentTitle"),
    description: t("sudoAutofill.notSentDescription"),
    variant: "destructive",
  })
}
