//! Camera and microphone commands. Capture is changed only by an explicit user action.
use crate::vbox::parse::VmState;
use crate::vbox::plan::VboxCommand;
use crate::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaAction {
    Microphone { enabled: bool },
    AttachCamera { alias: String },
    DetachCamera { alias: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Camera {
    pub alias: String,
    pub name: String,
}

pub fn valid_alias(alias: &str) -> bool {
    alias
        .strip_prefix('.')
        .is_some_and(|n| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

pub fn parse_cameras(text: &str) -> Vec<Camera> {
    text.lines()
        .filter_map(|line| {
            let (alias, name) = line.trim().split_once(char::is_whitespace)?;
            if !valid_alias(alias) {
                return None;
            }
            Some(Camera {
                alias: alias.into(),
                name: name.trim().trim_matches('"').into(),
            })
        })
        .collect()
}

pub fn command(uuid: &str, state: VmState, action: &MediaAction) -> Result<VboxCommand> {
    let (description, args): (&str, Vec<String>) = match action {
        MediaAction::Microphone { enabled } => {
            let switch = if *enabled { "on" } else { "off" };
            let args = match state {
                VmState::Running => vec!["controlvm", uuid, "audioin", switch],
                VmState::PoweredOff | VmState::Aborted => {
                    vec!["modifyvm", uuid, "--audio-in", switch]
                }
                _ => {
                    return Err(CoreError::InvalidInput(
                        "Start Windows or shut it down fully before changing the microphone."
                            .into(),
                    ))
                }
            };
            (
                if *enabled {
                    "Microphone input enabled"
                } else {
                    "Microphone input disabled"
                },
                args.into_iter().map(String::from).collect(),
            )
        }
        MediaAction::AttachCamera { alias } | MediaAction::DetachCamera { alias } => {
            if state != VmState::Running {
                return Err(CoreError::InvalidInput(
                    "Start Windows before connecting or disconnecting a camera.".into(),
                ));
            }
            if !valid_alias(alias) {
                return Err(CoreError::InvalidInput(
                    "Choose a camera alias from the device list.".into(),
                ));
            }
            let attach = matches!(action, MediaAction::AttachCamera { .. });
            let mut args = vec![
                "controlvm",
                uuid,
                "webcam",
                if attach { "attach" } else { "detach" },
                alias,
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
            if attach {
                args.push("MaxFramerate=30".into());
            }
            (
                if attach {
                    "Camera attached"
                } else {
                    "Camera detached"
                },
                args,
            )
        }
    };
    Ok(VboxCommand {
        description: description.into(),
        args,
        timeout_secs: 60,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_list_handles_mac_names_and_ignores_device_paths() {
        assert_eq!(
            parse_cameras("Video Input Devices: 1\n.1 \"FaceTime HD Camera\"\n0x12345\n"),
            vec![Camera {
                alias: ".1".into(),
                name: "FaceTime HD Camera".into()
            }]
        );
    }
    #[test]
    fn media_requires_appropriate_state_and_valid_device() {
        let action = MediaAction::AttachCamera { alias: ".1".into() };
        assert!(command("u", VmState::Saved, &action).is_err());
        assert_eq!(
            command("u", VmState::Running, &action).unwrap().args,
            vec![
                "controlvm",
                "u",
                "webcam",
                "attach",
                ".1",
                "MaxFramerate=30"
            ]
        );
        assert!(!valid_alias(".1; anything"));
        assert!(!valid_alias("."));
        let mic = MediaAction::Microphone { enabled: false };
        assert_eq!(
            command("u", VmState::Running, &mic).unwrap().args,
            vec!["controlvm", "u", "audioin", "off"]
        );
        assert_eq!(
            command("u", VmState::PoweredOff, &mic).unwrap().args,
            vec!["modifyvm", "u", "--audio-in", "off"]
        );
        assert!(command("u", VmState::Paused, &mic).is_err());
    }
}
