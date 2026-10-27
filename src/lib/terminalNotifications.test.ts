import { describe, expect, it } from "vitest"

import {
  localShellsMarkCommands,
  CommandTimer,
  decideAttention,
  formatCommandDuration,
  KittyNotificationAssembler,
  parseOsc777Notification,
  parseOsc9Notification,
  strongerAttention,
  type NotificationSettings,
} from "@/lib/terminalNotifications"

const settings: NotificationSettings = {
  notifications_enabled: true,
  notify_command_finished: true,
  notify_command_min_secs: 10,
  notify_terminal_requests: true,
  notify_agent_waiting: true,
  notify_agent_done: true,
  bell_notify: false,
}

describe("parseOsc9Notification", () => {
  it("reads a message", () => {
    expect(parseOsc9Notification("Claude is waiting for your input")).toEqual({
      body: "Claude is waiting for your input",
    })
  })

  it("leaves ConEmu's numbered commands alone", () => {
    expect(parseOsc9Notification("9;C:\\Users")).toBeNull()
    expect(parseOsc9Notification("4;1;50")).toBeNull()
    expect(parseOsc9Notification("4")).toBeNull()
  })

  it("drops control characters and empty messages", () => {
    expect(parseOsc9Notification("a\x07b\x1b")).toEqual({ body: "a b" })
    expect(parseOsc9Notification("  ")).toBeNull()
  })

  it("shortens long messages", () => {
    const body = parseOsc9Notification("x".repeat(5000))?.body ?? ""
    expect(body.length).toBe(1024)
    expect(body.endsWith("…")).toBe(true)
  })
})

describe("parseOsc777Notification", () => {
  it("reads title and body, keeping semicolons in the body", () => {
    expect(parseOsc777Notification("notify;Build;done; 3 warnings")).toEqual({
      title: "Build",
      body: "done; 3 warnings",
    })
  })

  it("shows a lone title as the message", () => {
    expect(parseOsc777Notification("notify;Build;")).toEqual({ body: "Build" })
  })

  it("ignores other commands", () => {
    expect(parseOsc777Notification("precmd")).toBeNull()
  })
})

describe("KittyNotificationAssembler", () => {
  it("shows a one-part notification as its title", () => {
    expect(new KittyNotificationAssembler().handle(";Hello")).toEqual({ body: "Hello" })
  })

  it("joins chunks with the same id", () => {
    const kitty = new KittyNotificationAssembler()
    expect(kitty.handle("i=1:d=0;Build")).toBeNull()
    expect(kitty.handle("i=1:p=body;finished")).toEqual({ title: "Build", body: "finished" })
  })

  it("decodes base64 payloads", () => {
    const encoded = btoa(String.fromCharCode(...new TextEncoder().encode("完成")))
    expect(new KittyNotificationAssembler().handle(`e=1;${encoded}`)).toEqual({ body: "完成" })
  })

  it("ignores queries and other payload types", () => {
    const kitty = new KittyNotificationAssembler()
    expect(kitty.handle("i=1:p=?;")).toBeNull()
    expect(kitty.handle("p=close;")).toBeNull()
    expect(kitty.handle("no separator")).toBeNull()
  })
})

describe("CommandTimer", () => {
  it("times a command from its output mark to its done mark", () => {
    const timer = new CommandTimer()
    timer.prompt()
    timer.setCommand("make")
    timer.start(1000)
    expect(timer.finish(0, 13_000)).toEqual({ command: "make", exitCode: 0, durationMs: 12_000 })
  })

  it("keeps the earliest start", () => {
    const timer = new CommandTimer()
    timer.start(1000)
    timer.start(1500)
    expect(timer.finish(1, 2000)?.durationMs).toBe(1000)
  })

  it("reports nothing for a done mark without a started command", () => {
    const timer = new CommandTimer()
    expect(timer.finish(0, 1000)).toBeNull()
    timer.start(1000)
    timer.prompt()
    expect(timer.finish(0, 2000)).toBeNull()
  })

  it("forgets the command once it finishes", () => {
    const timer = new CommandTimer()
    timer.setCommand("make")
    timer.start(0)
    timer.finish(0, 10)
    timer.start(20)
    expect(timer.finish(0, 30)?.command).toBeUndefined()
  })

  it("reports the first command started after the prompt until the prompt is back", () => {
    const running: (string | null)[] = []
    const timer = new CommandTimer((command) => running.push(command))
    timer.setTypedCommand("ls")
    timer.mark({ kind: "prompt" }, 0)
    timer.setTypedCommand("python")
    timer.start(10)
    timer.mark({ kind: "output" }, 20)
    expect(timer.mark({ kind: "done", exitCode: 0 }, 30)).toMatchObject({ exitCode: 0 })
    timer.mark({ kind: "prompt" }, 40)
    expect(running).toEqual(["python", null])
  })

  it("is not named by lines typed into the command it runs", () => {
    const running: (string | null)[] = []
    const timer = new CommandTimer((command) => running.push(command))
    timer.mark({ kind: "prompt" }, 0)
    // The command line was recalled from history, so nothing typed names it.
    timer.start(10)
    timer.setTypedCommand("fix the login bug")
    expect(running).toEqual([])
    expect(timer.finish(0, 30)?.command).toBeUndefined()
  })

  it("takes the command line the shell reports over what was typed", () => {
    const running: (string | null)[] = []
    const timer = new CommandTimer((command) => running.push(command))
    timer.mark({ kind: "prompt" }, 0)
    // Enter started it with a line completed with Tab, rebuilt as `cl`.
    timer.setTypedCommand("cl")
    timer.start(10)
    timer.setCommand("claude --resume")
    timer.setCommand("claude --resume")
    expect(running).toEqual(["cl", "claude --resume"])
    expect(timer.finish(0, 30)?.command).toBe("claude --resume")
  })
})

describe("decideAttention", () => {
  const longCommand = { kind: "command", exitCode: 0, durationMs: 30_000 } as const
  const request = { kind: "request", body: "hi" } as const
  const bell = { kind: "bell" } as const

  it("announces a pane in sight while the user works in another", () => {
    const besideFocus = { windowFocused: true, tabVisible: true, tabFocused: false }
    expect(decideAttention(request, settings, besideFocus)).toEqual({
      mark: null,
      system: false,
      toast: true,
    })
    expect(decideAttention(bell, settings, besideFocus)).toEqual({
      mark: null,
      system: false,
      toast: false,
    })
  })

  it("leaves a user looking at the tab alone", () => {
    for (const event of [longCommand, request, bell]) {
      expect(
        decideAttention(event, settings, {
          windowFocused: true,
          tabVisible: true,
          tabFocused: true,
        })
      ).toEqual({
        mark: null,
        system: false,
        toast: false,
      })
    }
  })

  it("notifies the system while the window is in the background", () => {
    expect(
      decideAttention(longCommand, settings, {
        windowFocused: false,
        tabVisible: true,
        tabFocused: true,
      })
    ).toEqual({ mark: null, system: true, toast: false })
    expect(
      decideAttention(request, settings, {
        windowFocused: false,
        tabVisible: false,
        tabFocused: false,
      })
    ).toEqual({ mark: "notification", system: true, toast: false })
  })

  it("announces agents by their own switches", () => {
    const hidden = { windowFocused: false, tabVisible: false, tabFocused: false }
    const waiting = { kind: "agent", agent: "claude", state: "waiting" } as const
    const done = { kind: "agent", agent: "claude", state: "done" } as const
    const error = { kind: "agent", agent: "claude", state: "error" } as const
    expect(decideAttention(waiting, settings, hidden)).toEqual({
      mark: "notification",
      system: true,
      toast: false,
    })
    const noWaiting = { ...settings, notify_agent_waiting: false }
    expect(decideAttention(waiting, noWaiting, hidden).mark).toBeNull()
    expect(decideAttention(done, noWaiting, hidden).system).toBe(true)
    const noDone = { ...settings, notify_agent_done: false }
    expect(decideAttention(done, noDone, hidden).mark).toBeNull()
    expect(decideAttention(error, noDone, hidden).mark).toBeNull()
    expect(decideAttention(waiting, noDone, hidden).system).toBe(true)
  })

  it("toasts and marks a hidden tab in a focused window", () => {
    expect(
      decideAttention(request, settings, {
        windowFocused: true,
        tabVisible: false,
        tabFocused: false,
      })
    ).toEqual({
      mark: "notification",
      system: false,
      toast: true,
    })
  })

  it("ignores short commands and turned off events", () => {
    const hidden = { windowFocused: false, tabVisible: false, tabFocused: false }
    const short = { ...longCommand, durationMs: 9_999 }
    expect(decideAttention(short, settings, hidden).mark).toBeNull()
    expect(
      decideAttention(longCommand, { ...settings, notify_command_finished: false }, hidden).mark
    ).toBeNull()
    expect(
      decideAttention(request, { ...settings, notify_terminal_requests: false }, hidden).mark
    ).toBeNull()
  })

  it("only marks tabs when announcements are off", () => {
    expect(
      decideAttention(
        request,
        { ...settings, notifications_enabled: false },
        { windowFocused: false, tabVisible: false, tabFocused: false }
      )
    ).toEqual({ mark: "notification", system: false, toast: false })
  })

  it("marks a bell and announces it only when asked to, never as a toast", () => {
    expect(
      decideAttention(bell, settings, {
        windowFocused: false,
        tabVisible: false,
        tabFocused: false,
      })
    ).toEqual({
      mark: "bell",
      system: false,
      toast: false,
    })
    const withBell = { ...settings, bell_notify: true }
    expect(
      decideAttention(bell, withBell, {
        windowFocused: false,
        tabVisible: false,
        tabFocused: false,
      }).system
    ).toBe(true)
    expect(
      decideAttention(bell, withBell, { windowFocused: true, tabVisible: false, tabFocused: false })
        .toast
    ).toBe(false)
  })
})

describe("strongerAttention", () => {
  it("keeps the strongest mark", () => {
    expect(strongerAttention(undefined, "activity")).toBe("activity")
    expect(strongerAttention("activity", "bell")).toBe("bell")
    expect(strongerAttention("notification", "activity")).toBe("notification")
  })
})

describe("formatCommandDuration", () => {
  it("formats seconds, minutes and hours", () => {
    expect(formatCommandDuration(42_400)).toBe("42s")
    expect(formatCommandDuration(185_000)).toBe("3m 5s")
    expect(formatCommandDuration(3_720_000)).toBe("1h 2m")
  })
})

describe("localShellsMarkCommands", () => {
  it("needs command marks everywhere and shell integration outside Windows", () => {
    const on = { command_marks: true, local_shell_integration: true }
    expect(localShellsMarkCommands(on, "macos")).toBe(true)
    expect(localShellsMarkCommands({ ...on, local_shell_integration: false }, "linux")).toBe(false)
    expect(localShellsMarkCommands({ ...on, local_shell_integration: false }, "windows")).toBe(true)
    expect(localShellsMarkCommands({ ...on, command_marks: false }, "windows")).toBe(false)
  })
})
