//! Lossless session decoding with typed views of recognized fields.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{Tab, TabConnection, TabType};

fn stringify(value: &Value) -> Result<String, serde_json::Error> {
    match value {
        Value::Number(number) => Ok(crate::workspace::js_number::format_f64(
            number.as_f64().unwrap_or(f64::NAN),
        )),
        Value::Array(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(stringify)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        )),
        Value::Object(fields) => {
            let mut entries: Vec<_> = fields.iter().collect();
            entries.sort_by_key(|(key, _)| {
                key.parse::<u32>()
                    .ok()
                    .filter(|index| *index != u32::MAX && index.to_string() == **key)
                    .map_or((1, 0), |index| (0, index))
            });
            let entries = entries
                .into_iter()
                .map(|(key, value)| {
                    Ok(format!(
                        "{}:{}",
                        serde_json::to_string(key)?,
                        stringify(value)?
                    ))
                })
                .collect::<Result<Vec<_>, serde_json::Error>>()?;
            Ok(format!("{{{}}}", entries.join(",")))
        }
        _ => serde_json::to_string(value),
    }
}

fn read<T: serde::de::DeserializeOwned>(fields: &Map<String, Value>, key: &str) -> Option<T> {
    fields
        .get(key)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn update<T: Serialize + PartialEq>(
    output: &mut Map<String, Value>,
    key: &str,
    current: &Option<T>,
    original: &Option<T>,
) -> Result<(), serde_json::Error> {
    if current != original || !output.contains_key(key) {
        if let Some(value) = current {
            output.insert(key.to_string(), serde_json::to_value(value)?);
        } else if current != original {
            output.shift_remove(key);
        }
    }
    Ok(())
}

macro_rules! connection_fields {
    ($action:ident, $($arguments:tt)*) => {
        $action!($($arguments)*;
            connection_type: "type", profile_id: "profileId", profile_name: "profileName",
            host: "host", port: "port", username: "username", password: "password",
            ignore_saved_password: "ignoreSavedPassword", remember_password: "rememberPassword",
            keepalive_interval_secs: "keepaliveIntervalSecs", keepalive_count_max: "keepaliveCountMax",
            server_monitor_visible: "serverMonitorVisible", auth_method: "authMethod",
            agent_forward: "agentForward", private_key_path: "privateKeyPath",
            private_key_passphrase: "privateKeyPassphrase", certificate_path: "certificatePath",
            terminal_shell: "terminalShell", terminal_shell_custom_path: "terminalShellCustomPath",
            terminal_shell_custom_args: "terminalShellCustomArgs", terminal_shell_id: "terminalShellId",
            terminal_shell_wsl_distro: "terminalShellWslDistro", cwd: "cwd",
            agent_resume: "agentResume", jump_hosts: "jumpHosts", encoding: "encoding",
            terminal_theme: "terminalTheme", login_script: "loginScript",
            ssh_shell_integration: "sshShellIntegration", session_log: "sessionLog",
            sftp_start_directory: "sftpStartDirectory", sftp_sudo: "sftpSudo",
            highlight_set: "highlightSet", proxy: "proxy", serial: "serial"
        )
    };
}

macro_rules! tab_fields {
    ($action:ident, $($arguments:tt)*) => {
        $action!($($arguments)*;
            has_connected: "hasConnected", is_modified: "isModified", icon: "icon", pid: "pid",
            session_nonce: "sessionNonce", connection_header_pinned: "connectionHeaderPinned",
            duplicate_base_title: "duplicateBaseTitle", auto_title: "autoTitle",
            shell_name: "shellName", icon_style: "iconStyle", distro_id: "distroId",
            quick_connect: "quickConnect", quick_connect_auth: "quickConnectAuth",
            settings_section: "settingsSection", settings_request: "settingsRequest",
            connection: "connection", remote_file: "remoteFile"
        )
    };
}

macro_rules! assign_fields {
    ($target:ident, $fields:ident; $($field:ident: $key:literal),*) => {
        $( $target.$field = read(&$fields, $key); )*
    };
}

macro_rules! update_fields {
    ($target:ident, $original:ident, $output:ident; $($field:ident: $key:literal),*) => {
        $( update(&mut $output, $key, &$target.$field, &$original.$field)?; )*
    };
}

impl TabConnection {
    fn from_fields(fields: Map<String, Value>) -> Self {
        let mut connection = Self::default();
        connection_fields!(assign_fields, connection, fields);
        connection.raw_fields = fields;
        connection
    }

    fn saved_value(&self) -> Result<Value, serde_json::Error> {
        let original = Self::from_fields(self.raw_fields.clone());
        let mut output = self.raw_fields.clone();
        connection_fields!(update_fields, self, original, output);
        Ok(Value::Object(output))
    }
}

impl Tab {
    fn from_fields(fields: Map<String, Value>) -> Self {
        let mut tab = Self::new(
            read::<String>(&fields, "id").unwrap_or_default(),
            read::<String>(&fields, "title").unwrap_or_default(),
            read::<TabType>(&fields, "type").unwrap_or(TabType::Unknown),
        );
        tab.is_active = read::<bool>(&fields, "isActive").unwrap_or(false);
        tab_fields!(assign_fields, tab, fields);
        tab.raw_fields = fields;
        tab.from_session = true;
        tab
    }

    fn saved_value(&self) -> Result<Value, serde_json::Error> {
        let original = Self::from_fields(self.raw_fields.clone());
        let mut output = self.raw_fields.clone();
        if !self.from_session
            || self.replaced_required_fields[0]
            || output.contains_key("id")
            || self.id != original.id
        {
            update(&mut output, "id", &Some(&self.id), &Some(&original.id))?;
        }
        if !self.from_session
            || self.replaced_required_fields[1]
            || output.contains_key("title")
            || self.title != original.title
        {
            update(
                &mut output,
                "title",
                &Some(&self.title),
                &Some(&original.title),
            )?;
        }
        if !self.from_session
            || self.replaced_required_fields[2]
            || output.contains_key("type")
            || self.tab_type != original.tab_type
        {
            update(
                &mut output,
                "type",
                &Some(self.tab_type),
                &Some(original.tab_type),
            )?;
        }
        if !self.from_session
            || self.replaced_required_fields[3]
            || output.contains_key("isActive")
            || self.is_active != original.is_active
        {
            update(
                &mut output,
                "isActive",
                &Some(self.is_active),
                &Some(original.is_active),
            )?;
        }
        tab_fields!(update_fields, self, original, output);
        Ok(Value::Object(output))
    }
}

macro_rules! lossless_serde {
    ($target:ty) => {
        impl<'de> Deserialize<'de> for $target {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = Value::deserialize(deserializer)?;
                let fields = value.as_object().cloned().unwrap_or_default();
                Ok(Self::from_fields(fields))
            }
        }

        impl Serialize for $target {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let text = self
                    .saved_value()
                    .and_then(|value| stringify(&value))
                    .map_err(serde::ser::Error::custom)?;
                serde_json::value::RawValue::from_string(text)
                    .map_err(serde::ser::Error::custom)?
                    .serialize(serializer)
            }
        }

        impl PartialEq for $target {
            fn eq(&self, other: &Self) -> bool {
                self.saved_value().ok() == other.saved_value().ok()
            }
        }
    };
}

lossless_serde!(TabConnection);
lossless_serde!(Tab);

impl Tab {
    /// The original JSON value of a field, including values outside its typed domain.
    pub fn saved_field(&self, key: &str) -> Option<&Value> {
        self.raw_fields.get(key)
    }
}

impl TabConnection {
    /// The original JSON value of a field, including unrecognized shell names.
    pub fn saved_field(&self, key: &str) -> Option<&Value> {
        self.raw_fields.get(key)
    }
}
