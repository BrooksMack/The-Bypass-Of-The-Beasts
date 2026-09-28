//! Guest network diagnosis. Pure decision logic over observable facts, so that repair actions
//! depend on the diagnosed problem rather than on guesses. The historical "UsbNcm Code 10"
//! scenario (guest loses networking after Windows Update on an Arm VM) is one branch.

use serde::{Deserialize, Serialize};

use crate::profile::GuestArch;

/// Facts the app can observe without guest credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkFacts {
    pub vm_running: bool,
    pub guest_additions_active: bool,
    /// `nic1` attachment ("nat", "none", ...).
    pub nic_attachment: Option<String>,
    /// `nictype1` ("usbnet", "virtio", "82540EM", ...).
    pub nic_type: Option<String>,
    pub cable_connected: Option<bool>,
    /// From `/VirtualBox/GuestInfo/Net/Count`.
    pub guest_adapter_count: Option<u32>,
    /// From `/VirtualBox/GuestInfo/Net/0/Status` ("Up"/"Down").
    pub guest_adapter_status: Option<String>,
    /// From `/VirtualBox/GuestInfo/Net/0/V4/IP`.
    pub guest_ipv4: Option<String>,
    /// The user confirmed a web page loaded inside Windows (the only honest internet check without guest credentials).
    pub user_confirmed_internet: Option<bool>,
    /// The user reported the adapter shows an error in Device Manager (e.g. "Code 10").
    pub user_reported_device_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkProblem {
    VmNotRunning,
    NoGuestAdditions,
    NoVirtualAdapter,
    CableDisconnected,
    AdapterMissingInGuest,
    DriverFailed,
    LinkDown,
    NoDhcpAddress,
    InternetUnverified,
    InternetUnavailable,
    Healthy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepairAction {
    StartVm,
    InstallGuestAdditions,
    AttachNat,
    ConnectCable,
    /// Toggle the virtual cable off/on to force the guest to renew its lease.
    ReconnectCable,
    /// Switch the emulated adapter (recorded so the original can be restored).
    SwitchNicType {
        from: Option<String>,
        to: String,
        requires_guest_driver: bool,
    },
    /// Guest-side step the user performs (plain language).
    GuestManual {
        title: String,
        steps: Vec<String>,
    },
    ConfirmInGuest,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub problem: NetworkProblem,
    pub summary: String,
    pub actions: Vec<RepairAction>,
}

pub fn diagnose(f: &NetworkFacts, guest: GuestArch) -> Diagnosis {
    if !f.vm_running {
        return Diagnosis {
            problem: NetworkProblem::VmNotRunning,
            summary: "Windows is not running, so its network cannot be checked yet.".into(),
            actions: vec![RepairAction::StartVm],
        };
    }
    match f.nic_attachment.as_deref() {
        Some("nat") | Some("natnetwork") | Some("bridged") | Some("hostonly") => {}
        _ => {
            return Diagnosis {
                problem: NetworkProblem::NoVirtualAdapter,
                summary: "The VM has no virtual network adapter attached.".into(),
                actions: vec![RepairAction::AttachNat],
            }
        }
    }
    if f.cable_connected == Some(false) {
        return Diagnosis {
            problem: NetworkProblem::CableDisconnected,
            summary: "The virtual network cable is unplugged.".into(),
            actions: vec![RepairAction::ConnectCable],
        };
    }
    if !f.guest_additions_active {
        return Diagnosis {
            problem: NetworkProblem::NoGuestAdditions,
            summary: "Guest Additions are not running inside Windows, so the network state cannot be read from outside. Check the network icon inside Windows.".into(),
            actions: vec![RepairAction::InstallGuestAdditions, RepairAction::ConfirmInGuest],
        };
    }

    let code10 = f
        .user_reported_device_error
        .as_deref()
        .map(|e| {
            e.to_ascii_lowercase().contains("code 10")
                || e.to_ascii_lowercase().contains("cannot start")
        })
        .unwrap_or(false);

    if f.guest_adapter_count == Some(0) || code10 {
        // Arm VMs default to the USB NCM adapter (Microsoft's UsbNcm driver). When that driver fails
        // after updates, the documented recovery is the Microsoft-signed ARM64 virtio-win NetKVM
        // driver plus switching the adapter to virtio-net. The original type is recorded for restore.
        if guest == GuestArch::Arm64
            && f.nic_type
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case("usbnet"))
                .unwrap_or(false)
        {
            return Diagnosis {
                problem: if code10 { NetworkProblem::DriverFailed } else { NetworkProblem::AdapterMissingInGuest },
                summary: if code10 {
                    "Windows' USB network driver (UsbNcm) failed to start (Code 10). This is a known problem after Windows updates on ARM VMs.".into()
                } else {
                    "Windows does not see a network adapter.".into()
                },
                actions: vec![
                    RepairAction::GuestManual {
                        title: "Try the simple fixes first".into(),
                        steps: vec![
                            "Restart Windows once.".into(),
                            "In Device Manager, right-click the network adapter and choose Uninstall device, then Action > Scan for hardware changes.".into(),
                        ],
                    },
                    RepairAction::GuestManual {
                        title: "If it still fails: install the virtio network driver".into(),
                        steps: vec![
                            "Download the stable virtio-win ISO (0.1.302 or newer) from the official virtio-win project inside Windows if you still have a connection, or on this computer and insert it as a disc.".into(),
                            "In Device Manager, update the failed adapter's driver from the ISO folder NetKVM\\w11\\ARM64. Windows must report the driver as signed by Microsoft; do not disable driver signature enforcement or Secure Boot.".into(),
                            "Shut Windows down, then use Switch adapter below.".into(),
                        ],
                    },
                    RepairAction::SwitchNicType { from: f.nic_type.clone(), to: "virtio".into(), requires_guest_driver: true },
                ],
            };
        }
        return Diagnosis {
            problem: NetworkProblem::AdapterMissingInGuest,
            summary: "Windows does not see a network adapter.".into(),
            actions: vec![
                RepairAction::InstallGuestAdditions,
                RepairAction::GuestManual {
                    title: "Check Device Manager".into(),
                    steps: vec!["Open Device Manager in Windows and look under Network adapters for a device with a warning icon.".into()],
                },
            ],
        };
    }

    if f.guest_adapter_status
        .as_deref()
        .map(|s| !s.eq_ignore_ascii_case("up"))
        .unwrap_or(false)
    {
        return Diagnosis {
            problem: NetworkProblem::LinkDown,
            summary: "The network adapter in Windows reports the link is down.".into(),
            actions: vec![RepairAction::ReconnectCable],
        };
    }

    match f.guest_ipv4.as_deref() {
        None | Some("") => Diagnosis {
            problem: NetworkProblem::NoDhcpAddress,
            summary: "Windows has not received a network address from VirtualBox's built-in NAT.".into(),
            actions: vec![
                RepairAction::ReconnectCable,
                RepairAction::GuestManual {
                    title: "Renew the address in Windows".into(),
                    steps: vec!["In Windows, open Settings > Network & internet and turn Ethernet off and on, or run Network troubleshooter.".into()],
                },
            ],
        },
        Some(ip) if ip.starts_with("169.254.") => Diagnosis {
            problem: NetworkProblem::NoDhcpAddress,
            summary: "Windows only has a self-assigned address (169.254.x.x); it did not reach the NAT DHCP server.".into(),
            actions: vec![RepairAction::ReconnectCable],
        },
        Some(_) => match f.user_confirmed_internet {
            Some(true) => Diagnosis {
                problem: NetworkProblem::Healthy,
                summary: "Network link is up, an address is assigned, and you confirmed a web page loads.".into(),
                actions: vec![RepairAction::None],
            },
            Some(false) => Diagnosis {
                problem: NetworkProblem::InternetUnavailable,
                summary: "Windows has an address but web pages do not load. This usually means this computer is offline, or a VPN/firewall on this computer blocks VirtualBox.".into(),
                actions: vec![
                    RepairAction::GuestManual {
                        title: "Check this computer's connection".into(),
                        steps: vec![
                            "Make sure this computer itself can load a web page.".into(),
                            "If you use a VPN, try disconnecting it and reloading the page inside Windows.".into(),
                        ],
                    },
                    RepairAction::ReconnectCable,
                ],
            },
            None => Diagnosis {
                problem: NetworkProblem::InternetUnverified,
                summary: "Windows has a network address. An address alone does not prove internet access, so please open a web page inside Windows to confirm.".into(),
                actions: vec![RepairAction::ConfirmInGuest],
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> NetworkFacts {
        NetworkFacts {
            vm_running: true,
            guest_additions_active: true,
            nic_attachment: Some("nat".into()),
            nic_type: Some("usbnet".into()),
            cable_connected: Some(true),
            guest_adapter_count: Some(1),
            guest_adapter_status: Some("Up".into()),
            guest_ipv4: Some("10.0.2.15".into()),
            user_confirmed_internet: None,
            user_reported_device_error: None,
        }
    }

    #[test]
    fn ip_alone_is_not_internet() {
        let d = diagnose(&base(), GuestArch::Arm64);
        assert_eq!(d.problem, NetworkProblem::InternetUnverified);
        assert_eq!(d.actions, vec![RepairAction::ConfirmInGuest]);
    }

    #[test]
    fn healthy_requires_confirmation() {
        let mut f = base();
        f.user_confirmed_internet = Some(true);
        assert_eq!(
            diagnose(&f, GuestArch::Arm64).problem,
            NetworkProblem::Healthy
        );
        f.user_confirmed_internet = Some(false);
        assert_eq!(
            diagnose(&f, GuestArch::Arm64).problem,
            NetworkProblem::InternetUnavailable
        );
    }

    #[test]
    fn usbncm_code10_on_arm_gets_targeted_recovery() {
        let mut f = base();
        f.guest_adapter_count = Some(0);
        f.guest_ipv4 = None;
        f.user_reported_device_error = Some("This device cannot start. (Code 10)".into());
        let d = diagnose(&f, GuestArch::Arm64);
        assert_eq!(d.problem, NetworkProblem::DriverFailed);
        assert!(d.actions.iter().any(|a| matches!(a, RepairAction::SwitchNicType { to, requires_guest_driver: true, from: Some(_) } if to == "virtio")));
        // Simple fixes come before the driver swap.
        assert!(matches!(d.actions[0], RepairAction::GuestManual { .. }));
    }

    #[test]
    fn missing_adapter_on_x64_does_not_suggest_arm_workaround() {
        let mut f = base();
        f.nic_type = Some("82540EM".into());
        f.guest_adapter_count = Some(0);
        let d = diagnose(&f, GuestArch::X64);
        assert_eq!(d.problem, NetworkProblem::AdapterMissingInGuest);
        assert!(!d
            .actions
            .iter()
            .any(|a| matches!(a, RepairAction::SwitchNicType { .. })));
    }

    #[test]
    fn ordering_of_checks() {
        let mut f = base();
        f.vm_running = false;
        assert_eq!(
            diagnose(&f, GuestArch::X64).problem,
            NetworkProblem::VmNotRunning
        );
        let mut f = base();
        f.nic_attachment = Some("none".into());
        assert_eq!(
            diagnose(&f, GuestArch::X64).problem,
            NetworkProblem::NoVirtualAdapter
        );
        let mut f = base();
        f.cable_connected = Some(false);
        assert_eq!(
            diagnose(&f, GuestArch::X64).problem,
            NetworkProblem::CableDisconnected
        );
        let mut f = base();
        f.guest_additions_active = false;
        assert_eq!(
            diagnose(&f, GuestArch::X64).problem,
            NetworkProblem::NoGuestAdditions
        );
        let mut f = base();
        f.guest_adapter_status = Some("Down".into());
        assert_eq!(
            diagnose(&f, GuestArch::X64).problem,
            NetworkProblem::LinkDown
        );
        let mut f = base();
        f.guest_ipv4 = Some("169.254.10.2".into());
        assert_eq!(
            diagnose(&f, GuestArch::X64).problem,
            NetworkProblem::NoDhcpAddress
        );
        let mut f = base();
        f.guest_ipv4 = None;
        assert_eq!(
            diagnose(&f, GuestArch::X64).problem,
            NetworkProblem::NoDhcpAddress
        );
    }
}
