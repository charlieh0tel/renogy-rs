//! Callsign newtypes: an [`Ssid`] station address and the base [`Callsign`]
//! derived from it.

use std::fmt;
use std::str::FromStr;

/// Error parsing a [`Callsign`] or [`Ssid`].
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("callsign is empty")]
    Empty,
}

/// Unconfigured placeholder callsign that the tool refuses to beacon under.
pub const PLACEHOLDER: &str = "N0CALL";

/// The base part of a station address, before any SSID suffix.
fn base_of(s: &str) -> &str {
    s.split('-').next().unwrap_or(s)
}

/// A licensed callsign with no SSID suffix (e.g. `N0CALL`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callsign(String);

impl Callsign {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True if this is the unconfigured placeholder call (`N0CALL`).
    #[must_use]
    pub fn is_placeholder(&self) -> bool {
        self.0 == PLACEHOLDER
    }
}

impl fmt::Display for Callsign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Callsign {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let base = base_of(s);
        if base.is_empty() {
            return Err(ParseError::Empty);
        }
        Ok(Callsign(base.to_string()))
    }
}

/// An APRS station address in `callsign-N` form (e.g. `N0CALL-11`), or a bare
/// callsign when no SSID is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ssid(String);

impl Ssid {
    /// The base callsign of this address, without the SSID suffix.
    #[must_use]
    pub fn base_call(&self) -> Callsign {
        Callsign(base_of(&self.0).to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Ssid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Ssid {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        Ok(Ssid(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::Callsign;
    use super::Ssid;

    #[test]
    fn ssid_base_call_strips_suffix() {
        let ssid: Ssid = "N0CALL-11".parse().unwrap();
        assert_eq!(ssid.base_call().as_str(), "N0CALL");
    }

    #[test]
    fn bare_callsign_passes_through() {
        let ssid: Ssid = "N0CALL".parse().unwrap();
        assert_eq!(ssid.base_call().as_str(), "N0CALL");
    }

    #[test]
    fn empty_ssid_is_rejected() {
        assert!("".parse::<Ssid>().is_err());
    }

    #[test]
    fn callsign_parse_strips_suffix() {
        assert_eq!("W1AW-12".parse::<Callsign>().unwrap().as_str(), "W1AW");
    }

    #[test]
    fn placeholder_is_detected_through_ssid() {
        let ssid: Ssid = "N0CALL-1".parse().unwrap();
        assert!(ssid.base_call().is_placeholder());
        let real: Ssid = "W1AW-12".parse().unwrap();
        assert!(!real.base_call().is_placeholder());
    }
}
