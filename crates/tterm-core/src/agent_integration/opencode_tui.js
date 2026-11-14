// tTerm agent integration for OpenCode 2. tTerm installs this file from
// Settings > Notifications and removes it from there; edits are overwritten.
//
// Reports what OpenCode is doing to the terminal it runs in, as
// OSC 777 ; tterm-agent ; opencode ; <state> BEL, for tTerm to show on the
// tab and announce. OpenCode 2 runs sessions in a background service shared
// by every terminal, so this is a TUI plugin: it runs in the TUI process,
// which owns the terminal and knows the session on screen.

import { writeSync } from "node:fs";

/** Synchronous, so the report on exit is out before the process is gone. */
function report(state) {
  let sequence = `\x1b]777;tterm-agent;opencode;${state}\x07`;
  // tmux passes it on only wrapped, and with allow-passthrough on.
  if (process.env.TMUX) sequence = `\x1bPtmux;\x1b${sequence}\x1b\\`;
  try {
    writeSync(1, sequence);
  } catch {
    // The terminal is gone.
  }
}

export default {
  id: "tterm-agent",
  setup(ctx) {
    /** The root session on screen, or null. */
    let root = null;
    /** The state last reported. */
    let reported = null;
    /** How the root session's last run ended: succeeded, failed or interrupted. */
    let outcome = null;

    function currentRoot() {
      const route = ctx.ui.router.current();
      // `--continue` starts on a placeholder session.
      if (route?.type !== "session" || !route.sessionID || route.sessionID === "dummy") {
        return null;
      }
      return ctx.data.session.root(route.sessionID) || route.sessionID;
    }

    /** The session and its subagents. */
    function family(id) {
      return [id, ...(ctx.data.session.family(id) ?? []).filter((member) => member !== id)];
    }

    function activity(id) {
      const members = family(id);
      const asking = members.some(
        (member) =>
          (ctx.data.session.permission.list(member)?.length ?? 0) > 0 ||
          (ctx.data.session.form.list(member)?.length ?? 0) > 0
      );
      if (asking) return "waiting";
      return members.some((member) => ctx.data.session.status(member) === "running")
        ? "processing"
        : "idle";
    }

    function send(state) {
      if (state === reported) return;
      reported = state;
      report(state);
    }

    function refresh() {
      const next = currentRoot();
      if (next !== root) {
        root = next;
        outcome = null;
        // Opening or leaving a session finishes nothing.
        send(root ? activity(root) : "idle");
        if (root) {
          // Pending requests are loaded on demand.
          Promise.all(
            family(root).flatMap((member) => [
              ctx.data.session.permission.sync(member),
              ctx.data.session.form.sync(member),
            ])
          )
            .catch(() => {})
            .then(refresh);
        }
        return;
      }
      if (!root) return;
      const state = activity(root);
      if (state === reported) return;
      reported = state;
      if (state !== "idle") {
        report(state);
        return;
      }
      // An interruption is the user's own doing; it finishes nothing.
      report(outcome === "failed" ? "error" : outcome === "interrupted" ? "idle" : "done");
    }

    const stopListening = ctx.data.listen(({ details }) => {
      const type = details?.type ?? "";
      if (type.startsWith("session.execution.") && details.data?.sessionID === root) {
        outcome = type.slice("session.execution.".length);
      }
      if (/^(session\.execution\.|session\.retry\.|permission\.|form\.)/.test(type)) {
        // OpenCode updates its stores from the same event; read them after.
        setTimeout(refresh, 0);
      }
    });
    // The route has no change event a plugin can follow.
    const timer = setInterval(refresh, 500);
    timer.unref?.();
    // OpenCode disposes its plugins as it quits, but may also exit without.
    const ended = () => report("ended");
    process.once("exit", ended);
    refresh();

    // A plugin reloaded while the TUI runs reports the state again at once.
    return () => {
      clearInterval(timer);
      stopListening();
      process.removeListener("exit", ended);
      ended();
    };
  },
};
