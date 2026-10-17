// The sound a system notification makes when it is not the system's own. A
// Windows toast plays only the system's sounds for an app that is not
// packaged, so tTerm shows the notification silently and plays this itself.
import { invoke } from "@tauri-apps/api/core"

import type { AppConfig, NotificationSoundName } from "@/lib/appConfig"

/** One note of a tone tTerm synthesizes, so no sound file ships. */
export interface ToneNote {
  /** In Hz. */
  frequency: number
  /** Slides the pitch here over the note, in Hz. */
  glideTo?: number
  /** Seconds after the tone starts. */
  at: number
  /** Seconds the note takes to fade out. */
  decay: number
  /** Peak gain, from 0 to 1. */
  gain: number
}

export type ToneName = Exclude<NotificationSoundName, "system" | "custom">

export const TONES: Record<ToneName, ToneNote[]> = {
  // Two notes a fifth apart, the second rising over the first.
  chime: [
    { frequency: 880, at: 0, decay: 0.5, gain: 0.12 },
    { frequency: 1318.5, at: 0.12, decay: 0.7, gain: 0.12 },
  ],
  // A small bell: its overtones are what make it sound struck.
  ding: [
    { frequency: 1046.5, at: 0, decay: 0.9, gain: 0.14 },
    { frequency: 2093, at: 0, decay: 0.4, gain: 0.03 },
    { frequency: 2890, at: 0, decay: 0.25, gain: 0.015 },
  ],
  pop: [{ frequency: 520, glideTo: 1100, at: 0, decay: 0.12, gain: 0.16 }],
  rise: [
    { frequency: 784, at: 0, decay: 0.3, gain: 0.1 },
    { frequency: 988, at: 0.09, decay: 0.3, gain: 0.1 },
    { frequency: 1175, at: 0.18, decay: 0.5, gain: 0.1 },
  ],
}

export type NotificationSoundSettings = Pick<
  AppConfig,
  "notify_sound" | "notify_sound_name" | "notify_sound_file"
>

/** What makes a notification's sound. */
export type NotificationSoundSource =
  { kind: "none" } | { kind: "system" } | { kind: "tone"; notes: ToneNote[] } | { kind: "file" }

export function notificationSoundSource(
  settings: NotificationSoundSettings
): NotificationSoundSource {
  if (!settings.notify_sound) return { kind: "none" }
  const name = settings.notify_sound_name
  if (name === "system") return { kind: "system" }
  if (name === "custom") {
    return settings.notify_sound_file.trim() ? { kind: "file" } : { kind: "system" }
  }
  return { kind: "tone", notes: TONES[name] }
}

/** Copies a sound file in as tTerm's notification sound; resolves to its name. */
export function importNotificationSound(path: string) {
  return invoke<string>("import_notification_sound", { path })
}

let context: AudioContext | null = null

function audioContext() {
  context ??= new AudioContext()
  if (context.state === "suspended") void context.resume()
  return context
}

function playTone(context: AudioContext, notes: ToneNote[]) {
  const start = context.currentTime
  for (const note of notes) {
    const at = start + note.at
    const oscillator = context.createOscillator()
    const gain = context.createGain()
    oscillator.type = "sine"
    oscillator.frequency.setValueAtTime(note.frequency, at)
    if (note.glideTo)
      oscillator.frequency.exponentialRampToValueAtTime(note.glideTo, at + note.decay)
    gain.gain.setValueAtTime(0.0001, at)
    gain.gain.exponentialRampToValueAtTime(note.gain, at + 0.01)
    gain.gain.exponentialRampToValueAtTime(0.0001, at + note.decay)
    oscillator.connect(gain).connect(context.destination)
    oscillator.start(at)
    oscillator.stop(at + note.decay + 0.02)
  }
}

/**
 * Plays a tone or tTerm's copy of the sound file now. The copy is read each
 * time, since a sync or a restored backup may have replaced it. Rejects when
 * it cannot be read or decoded; the settings show why.
 */
export async function playNotificationSoundSource(source: NotificationSoundSource) {
  if (source.kind === "tone") {
    playTone(audioContext(), source.notes)
  } else if (source.kind === "file") {
    const context = audioContext()
    const bytes = await invoke<ArrayBuffer>("read_notification_sound")
    const player = context.createBufferSource()
    player.buffer = await context.decodeAudioData(bytes)
    player.connect(context.destination)
    player.start()
  }
}

/**
 * Plays the sound the settings pick for a notification, unless that is the
 * system's. Resolves whether the system notification should play its own
 * sound: when it is the one picked, or when the picked file cannot be played.
 */
export async function playNotificationSound(settings: NotificationSoundSettings) {
  const source = notificationSoundSource(settings)
  if (source.kind === "none") return false
  if (source.kind === "system") return true
  try {
    await playNotificationSoundSource(source)
    return false
  } catch (error) {
    console.warn("Failed to play the notification sound:", error)
    return true
  }
}
