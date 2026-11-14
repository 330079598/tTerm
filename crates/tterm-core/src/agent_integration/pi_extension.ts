// tTerm agent integration for pi. tTerm installs this file from
// Settings > Notifications and removes it from there; edits are overwritten.
//
// Reports what pi is doing to the terminal it runs in, as
// OSC 777 ; tterm-agent ; pi ; <state> BEL, for tTerm to show on the tab and
// announce. pi draws its interface on stdout, so the report goes there too.

function report(state) {
  if (!process.stdout.isTTY) return;
  let sequence = `\x1b]777;tterm-agent;pi;${state}\x07`;
  // tmux passes it on only wrapped, and with allow-passthrough on.
  if (process.env.TMUX) sequence = `\x1bPtmux;\x1b${sequence}\x1b\\`;
  try {
    process.stdout.write(sequence);
  } catch {
    // The terminal is gone.
  }
}

export default function (pi) {
  let running = false;
  /** Why the run's last reply stopped: "aborted", "error", or another. */
  let stopReason = null;

  pi.on("agent_start", async () => {
    running = true;
    stopReason = null;
    report("processing");
  });
  // A run may continue after this (a retry, a queued message); it is over at
  // agent_settled.
  pi.on("agent_end", async (event) => {
    const reply = event.messages?.findLast?.((message) => message?.role === "assistant");
    stopReason = reply?.stopReason ?? null;
  });
  pi.on("agent_settled", async () => {
    running = false;
    // An abort is the user's own doing; it finishes nothing.
    report(stopReason === "error" ? "error" : stopReason === "aborted" ? "idle" : "done");
  });
  // An extension asking the user (a confirmation, a choice).
  pi.on("ui_prompt_start", async () => report("waiting"));
  pi.on("ui_prompt_end", async () => report(running ? "processing" : "idle"));
  pi.on("session_shutdown", async (event) => {
    if (event.reason === "quit") report("ended");
  });
}
