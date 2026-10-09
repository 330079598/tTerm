//! Hooks in an agent's JSON settings, in the shape Claude Code and Codex
//! share: `hooks.<Event>` lists groups, each with a list of handlers. tTerm
//! adds one group per event and recognizes its handlers by the report they
//! write, leaving every other hook as it is.

use super::json::{Json, Object};
use super::{IntegrationState, REPORT_MARKER};

/// The hooks tTerm installs into one agent.
pub(super) struct HookSet {
    /// The settings file under the agent's config directory.
    pub file: &'static str,
    /// Each hook event tTerm listens to and the state it reports.
    pub events: &'static [(&'static str, &'static str)],
    /// The handler's command reporting a state.
    pub command: fn(&str) -> String,
}

impl HookSet {
    fn group(&self, state: &str) -> Json {
        let mut handler = Object::new();
        handler.insert("type".to_string(), Json::String("command".to_string()));
        handler.insert("command".to_string(), Json::String((self.command)(state)));
        let mut group = Object::new();
        group.insert(
            "hooks".to_string(),
            Json::Array(vec![Json::Object(handler)]),
        );
        Json::Object(group)
    }

    /// The `hooks` section tTerm adds, for the user to review.
    pub(super) fn preview(&self) -> Object {
        let mut hooks = Object::new();
        for (event, state) in self.events {
            hooks.insert(event.to_string(), Json::Array(vec![self.group(state)]));
        }
        let mut root = Object::new();
        root.insert("hooks".to_string(), Json::Object(hooks));
        root
    }

    pub(super) fn state(&self, settings: &Object) -> IntegrationState {
        let Some(Json::Object(hooks)) = settings.get("hooks") else {
            return IntegrationState::NotInstalled;
        };
        // Every group holding one of our handlers, by event.
        let mut found: Vec<(&str, &Json)> = Vec::new();
        for (event, groups) in hooks {
            let Json::Array(groups) = groups else {
                continue;
            };
            for group in groups {
                if group_handlers(group).is_some_and(|handlers| handlers.iter().any(is_ours)) {
                    found.push((event, group));
                }
            }
        }
        if found.is_empty() {
            return IntegrationState::NotInstalled;
        }
        let current = found.len() == self.events.len()
            && self.events.iter().all(|(event, state)| {
                let expected = self.group(state);
                found
                    .iter()
                    .any(|(found_event, group)| found_event == event && **group == expected)
            });
        if current {
            IntegrationState::Installed
        } else {
            IntegrationState::Outdated
        }
    }

    /// Drops our handlers, then the groups, events and `hooks` section they leave
    /// empty. Everything else stays as it was.
    pub(super) fn uninstall(&self, settings: &mut Object) {
        let Some(Json::Object(hooks)) = settings.get_mut("hooks") else {
            return;
        };
        let mut removed = false;
        hooks.retain(|_, groups| {
            let Json::Array(list) = groups else {
                return true;
            };
            let before = list.len();
            list.retain_mut(|group| {
                let Json::Object(group) = group else {
                    return true;
                };
                let Some(Json::Array(handlers)) = group.get_mut("hooks") else {
                    return true;
                };
                let count = handlers.len();
                handlers.retain(|handler| !is_ours(handler));
                if handlers.len() == count {
                    return true;
                }
                removed = true;
                !handlers.is_empty()
            });
            // Only our removals shrink the list.
            list.len() == before || !list.is_empty()
        });
        if removed && hooks.is_empty() {
            settings.shift_remove("hooks");
        }
    }

    pub(super) fn install(&self, settings: &mut Object) -> Result<(), String> {
        self.uninstall(settings);
        let hooks = settings
            .entry("hooks".to_string())
            .or_insert_with(|| Json::Object(Object::new()));
        let Json::Object(hooks) = hooks else {
            return Err(format!("\"hooks\" in {} is not an object", self.file));
        };
        for (event, _) in self.events {
            if hooks
                .get(*event)
                .is_some_and(|groups| !matches!(groups, Json::Array(_)))
            {
                return Err(format!("\"hooks.{event}\" in {} is not a list", self.file));
            }
        }
        for (event, state) in self.events {
            let groups = hooks
                .entry(event.to_string())
                .or_insert_with(|| Json::Array(Vec::new()));
            if let Json::Array(groups) = groups {
                groups.push(self.group(state));
            }
        }
        Ok(())
    }
}

fn is_ours(handler: &Json) -> bool {
    match handler {
        Json::Object(handler) => handler
            .get("command")
            .and_then(Json::as_str)
            .is_some_and(|command| command.contains(REPORT_MARKER)),
        _ => false,
    }
}

fn group_handlers(group: &Json) -> Option<&Vec<Json>> {
    match group {
        Json::Object(group) => match group.get("hooks") {
            Some(Json::Array(handlers)) => Some(handlers),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::claude_code::HOOKS;
    use super::super::json::{parse_object, to_text};
    use super::*;

    const USER_SETTINGS: &str = r#"{
  "model": "opus",
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "afplay /System/Library/Sounds/Glass.aiff"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/bin/check.sh"
          }
        ]
      }
    ]
  },
  "permissions": {
    "allow": [
      "Bash(ls:*)"
    ]
  }
}
"#;

    #[test]
    fn installs_next_to_the_users_hooks() {
        let mut settings = parse_object(USER_SETTINGS).unwrap();
        assert_eq!(HOOKS.state(&settings), IntegrationState::NotInstalled);
        HOOKS.install(&mut settings).unwrap();
        assert_eq!(HOOKS.state(&settings), IntegrationState::Installed);

        let Some(Json::Object(hooks)) = settings.get("hooks") else {
            panic!()
        };
        let Some(Json::Array(stop)) = hooks.get("Stop") else {
            panic!()
        };
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[1], HOOKS.group("done"));
        // Keys keep their places; new events follow the user's.
        let keys: Vec<_> = settings.keys().cloned().collect();
        assert_eq!(keys, ["model", "hooks", "permissions"]);
        let events: Vec<_> = hooks.keys().take(3).cloned().collect();
        assert_eq!(events, ["Stop", "PreToolUse", "UserPromptSubmit"]);
    }

    #[test]
    fn installing_twice_changes_nothing() {
        let mut settings = parse_object(USER_SETTINGS).unwrap();
        HOOKS.install(&mut settings).unwrap();
        let once = to_text(&settings);
        HOOKS.install(&mut settings).unwrap();
        assert_eq!(to_text(&settings), once);
    }

    #[test]
    fn uninstall_restores_the_users_file() {
        let mut settings = parse_object(USER_SETTINGS).unwrap();
        HOOKS.install(&mut settings).unwrap();
        HOOKS.uninstall(&mut settings);
        assert_eq!(to_text(&settings), USER_SETTINGS);
        assert_eq!(HOOKS.state(&settings), IntegrationState::NotInstalled);
    }

    #[test]
    fn uninstall_drops_the_hooks_section_it_created() {
        let mut settings = parse_object("{\"model\": \"opus\"}").unwrap();
        HOOKS.install(&mut settings).unwrap();
        HOOKS.uninstall(&mut settings);
        assert_eq!(to_text(&settings), "{\n  \"model\": \"opus\"\n}\n");
    }

    #[test]
    fn uninstall_keeps_empty_sections_the_user_wrote() {
        let mut settings = parse_object("{\"hooks\": {\"Stop\": []}}").unwrap();
        HOOKS.uninstall(&mut settings);
        assert_eq!(
            to_text(&settings),
            "{\n  \"hooks\": {\n    \"Stop\": []\n  }\n}\n"
        );
    }

    #[test]
    fn a_changed_or_partial_install_is_outdated() {
        let mut settings = parse_object("{}").unwrap();
        HOOKS.install(&mut settings).unwrap();
        let Some(Json::Object(hooks)) = settings.get_mut("hooks") else {
            panic!()
        };
        hooks.shift_remove("Elicitation");
        assert_eq!(HOOKS.state(&settings), IntegrationState::Outdated);

        HOOKS.install(&mut settings).unwrap();
        let Some(Json::Object(hooks)) = settings.get_mut("hooks") else {
            panic!()
        };
        let Some(Json::Array(stop)) = hooks.get_mut("Stop") else {
            panic!()
        };
        stop[0] = HOOKS.group("processing");
        assert_eq!(HOOKS.state(&settings), IntegrationState::Outdated);
    }

    #[test]
    fn refuses_hooks_it_cannot_extend() {
        let mut settings = parse_object("{\"hooks\": []}").unwrap();
        assert!(HOOKS.install(&mut settings).is_err());
        let mut settings = parse_object("{\"hooks\": {\"Stop\": {}}}").unwrap();
        assert!(HOOKS.install(&mut settings).is_err());
        assert_eq!(
            to_text(&settings),
            "{\n  \"hooks\": {\n    \"Stop\": {}\n  }\n}\n"
        );
    }
}
