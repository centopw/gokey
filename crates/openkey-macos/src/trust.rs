//! The Accessibility trust state: what the app is allowed to do, and what to
//! tell the user when it is not allowed to do it.
//!
//! ADR-0004 consequence 1 makes TCC a first-class UI state, not an error path.
//! There are three states and all three are reachable on a normal Mac:
//!
//! | state | what the user sees | what the app does |
//! | --- | --- | --- |
//! | not determined | first run, nothing granted yet | banner + deep link, no interception |
//! | denied | the grant was refused or revoked | banner naming the pane, no interception |
//! | granted | normal operation | tap installed |
//!
//! ## Why "not determined" is a proxy
//!
//! Accessibility has no "has the user been asked?" query. `AXIsProcessTrusted`
//! answers granted/not-granted and nothing else. `IOHIDCheckAccess` does
//! distinguish *unknown* (never requested) from *denied*, so this module uses it
//! to tell the two apart, and says so here rather than presenting a guess as a
//! reading. If the HID axes are both unknown and Accessibility is not trusted,
//! the state is not-determined; if either axis is denied, the user has made a
//! decision and the state is denied.
//!
//! The practical difference is only copy: both states install no tap and both
//! open the same pane. The distinction matters because "not determined" is the
//! state a new user is in, and a one-shot modal at launch is exactly what
//! ADR-0004 rejected.

use std::fmt;

/// `IOHIDAccessType` — also the shape of `IOHIDCheckAccess`'s return value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Granted,
    Denied,
    Unknown,
}

impl Access {
    /// Map `IOHIDCheckAccess`'s raw value. Anything outside the documented
    /// 0/1/2 is reported as `Unknown` rather than folded into `Granted`: an
    /// unrecognised answer must never read as permission.
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::Granted,
            1 => Self::Denied,
            _ => Self::Unknown,
        }
    }
}

/// The three values the system will answer, as read at one instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessReport {
    /// `AXIsProcessTrusted()`. An active tap needs this one.
    pub accessibility: bool,
    /// `IOHIDCheckAccess(kIOHIDRequestTypePostEvent)`.
    pub hid_post: Access,
    /// `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)`.
    pub hid_listen: Access,
}

impl AccessReport {
    pub fn trust(&self) -> TrustState {
        if self.accessibility {
            TrustState::Granted
        } else if self.hid_post == Access::Denied || self.hid_listen == Access::Denied {
            TrustState::Denied
        } else {
            TrustState::NotDetermined
        }
    }
}

impl Default for AccessReport {
    fn default() -> Self {
        // Nothing granted, nothing asked. This is the first-run value, and the
        // only safe default for a permission report.
        Self {
            accessibility: false,
            hid_post: Access::Unknown,
            hid_listen: Access::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustState {
    NotDetermined,
    Denied,
    Granted,
}

impl TrustState {
    pub fn is_granted(self) -> bool {
        self == Self::Granted
    }

    /// Stable token for logs and for the menu bar (S-08). Never translated.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotDetermined => "not_determined",
            Self::Denied => "denied",
            Self::Granted => "granted",
        }
    }
}

impl fmt::Display for TrustState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Deep link into System Settings → Privacy & Security → Accessibility.
///
/// macOS accepts the `x-apple.systempreferences:` scheme with a pane anchor. The
/// anchor names the privacy *category*, and the OS opens the current pane id for
/// it across 13 and later, which is why the older `com.apple.preference.security`
/// bundle path is still the documented form.
pub const ACCESSIBILITY_PANE_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// The second axis that may apply at the HID tap location (`Privacy_ListenEvent`).
/// Offering it is honest about what has not been measured yet: the trusted run in
/// ADR-0004 is what decides whether it is required.
pub const INPUT_MONITORING_PANE_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

/// Banner content for S-09. Present for every state where the app is not
/// intercepting; absent when it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Banner {
    pub state: TrustState,
    pub title: &'static str,
    pub body: &'static str,
    /// Button that opens [`Self::url`].
    pub action: &'static str,
    pub url: &'static str,
    /// A second pane worth offering, labelled. `None` when there is nothing
    /// beyond the primary link.
    pub secondary: Option<(&'static str, &'static str)>,
    /// What happens after the grant, stated where the user is looking.
    pub after: &'static str,
}

const AFTER_GRANT: &str = "Grant access, then come back to this window: GoKey re-checks the \
                           permission without a restart.";

/// Copy for the three states. `Granted` returns `None` because there is nothing
/// to warn about — a banner that is always present is a banner nobody reads.
pub fn banner(state: TrustState) -> Option<Banner> {
    let (title, body) = match state {
        TrustState::Granted => return None,
        TrustState::NotDetermined => (
            "GoKey needs Accessibility access",
            "macOS is not letting GoKey read or replace keystrokes yet, so Vietnamese input is \
             off. Nothing is being swallowed: every key reaches the application untouched.",
        ),
        TrustState::Denied => (
            "GoKey cannot convert keystrokes",
            "Accessibility access was denied or revoked, so GoKey is not intercepting anything \
             and typing works normally but uncorrected. Re-enable GoKey in the list on the right.",
        ),
    };
    Some(Banner {
        state,
        title,
        body,
        action: "Open System Settings",
        url: ACCESSIBILITY_PANE_URL,
        secondary: Some(("Also check Input Monitoring", INPUT_MONITORING_PANE_URL)),
        after: AFTER_GRANT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(accessibility: bool, post: Access, listen: Access) -> AccessReport {
        AccessReport {
            accessibility,
            hid_post: post,
            hid_listen: listen,
        }
    }

    #[test]
    fn every_report_maps_to_one_state() {
        let unknown = Access::Unknown;
        let cases = [
            // Accessibility is the grant an active tap needs; when it is there,
            // the HID answers cannot downgrade it.
            (report(true, unknown, unknown), TrustState::Granted),
            (
                report(true, Access::Denied, Access::Denied),
                TrustState::Granted,
            ),
            // Untrusted and nothing ever asked: first run.
            (report(false, unknown, unknown), TrustState::NotDetermined),
            (
                report(false, Access::Granted, unknown),
                TrustState::NotDetermined,
            ),
            (
                report(false, unknown, Access::Granted),
                TrustState::NotDetermined,
            ),
            // Untrusted with a recorded refusal: the user decided.
            (report(false, Access::Denied, unknown), TrustState::Denied),
            (report(false, unknown, Access::Denied), TrustState::Denied),
            (
                report(false, Access::Denied, Access::Denied),
                TrustState::Denied,
            ),
            (
                report(false, Access::Granted, Access::Denied),
                TrustState::Denied,
            ),
        ];
        assert_eq!(cases.len(), 9);
        for (report, expected) in cases {
            assert_eq!(report.trust(), expected, "{report:?}");
        }
    }

    #[test]
    fn the_default_report_is_first_run_not_granted() {
        let report = AccessReport::default();
        assert_eq!(report.trust(), TrustState::NotDetermined);
        assert!(!report.accessibility);
    }

    #[test]
    fn unrecognised_hid_answers_are_unknown_not_granted() {
        assert_eq!(Access::from_raw(0), Access::Granted);
        assert_eq!(Access::from_raw(1), Access::Denied);
        assert_eq!(Access::from_raw(2), Access::Unknown);
        assert_eq!(Access::from_raw(7), Access::Unknown);
        assert_eq!(Access::from_raw(u32::MAX), Access::Unknown);
    }

    #[test]
    fn every_non_granted_state_has_a_banner_with_a_working_link() {
        for state in [TrustState::NotDetermined, TrustState::Denied] {
            let banner = banner(state).expect("a banner");
            assert_eq!(banner.state, state);
            assert!(!banner.title.is_empty() && !banner.body.is_empty());
            assert!(banner.url.starts_with("x-apple.systempreferences:"));
            assert!(banner.url.contains("Privacy_Accessibility"));
            assert!(banner.after.contains("re-checks"));
            assert!(banner.after.contains("without a restart"));
            // The copy may promise the re-check; it must not promise that
            // interception starts on its own, which only a new tap install does.
            assert!(!banner.after.contains("starts"));
            assert!(!banner.after.contains("intercepting on its own"));
            let (_, secondary) = banner.secondary.expect("input monitoring offered");
            assert!(secondary.contains("Privacy_ListenEvent"));
            assert_ne!(banner.url, secondary, "the two panes are distinct links");
        }
        assert!(banner(TrustState::Granted).is_none());
    }

    #[test]
    fn the_two_states_read_differently() {
        let first_run = banner(TrustState::NotDetermined).unwrap();
        let denied = banner(TrustState::Denied).unwrap();
        assert_ne!(first_run.title, denied.title);
        assert_ne!(first_run.body, denied.body);
    }

    #[test]
    fn state_tokens_are_stable() {
        assert_eq!(TrustState::NotDetermined.as_str(), "not_determined");
        assert_eq!(TrustState::Denied.as_str(), "denied");
        assert_eq!(TrustState::Granted.as_str(), "granted");
        assert_eq!(TrustState::Denied.to_string(), "denied");
    }
}
