// The terminal bell as a short tone, synthesized so no sound file ships.
let context: AudioContext | null = null
let lastPlayedAt = 0

/** Bells closer together than this ring once. */
const MIN_INTERVAL_MS = 150

export function playBellSound() {
  const now = Date.now()
  if (now - lastPlayedAt < MIN_INTERVAL_MS) return
  lastPlayedAt = now
  try {
    context ??= new AudioContext()
    if (context.state === "suspended") void context.resume()
    const start = context.currentTime
    const oscillator = context.createOscillator()
    const gain = context.createGain()
    oscillator.type = "sine"
    oscillator.frequency.value = 880
    gain.gain.setValueAtTime(0.0001, start)
    gain.gain.exponentialRampToValueAtTime(0.15, start + 0.01)
    gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.18)
    oscillator.connect(gain).connect(context.destination)
    oscillator.start(start)
    oscillator.stop(start + 0.2)
  } catch (error) {
    console.warn("Failed to play the terminal bell:", error)
  }
}
