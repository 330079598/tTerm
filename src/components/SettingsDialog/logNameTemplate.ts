/** A whole number in `0..=max` typed into a field, or null. */
export function parseLimit(value: string, max: number): number | null {
  const parsed = Number(value)
  return Number.isInteger(parsed) && parsed >= 0 && parsed <= max ? parsed : null
}

/** Folders a name template may put logs in, as the backend allows. */
export const MAX_LOG_FOLDERS = 5

/** The template's folders and file name; `/` and `\\` both separate them. */
export function templateParts(template: string) {
  return template.split(/[/\\]/).filter((part) => part.trim())
}

export function renderExample(template: string) {
  return [
    ["{profile}", "production"],
    ["{host}", "10.0.0.8"],
    ["{port}", "22"],
    ["{username}", "root"],
    ["{type}", "ssh"],
    ["{year}", "2026"],
    ["{month}", "07"],
    ["{day}", "29"],
    ["{date}", "20260729"],
    ["{time}", "153012"],
    ["{yyyyMMdd-HHmmss}", "20260729-153012"],
    ["{sessionId}", "a83f"],
  ].reduce(
    (value, [token, replacement]) => value.split(token).join(replacement),
    templateParts(template).join("/")
  )
}
