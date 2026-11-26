// SPDX-License-Identifier: AGPL-3.0-or-later
//! Structured errors returned by the validating constructors and formatters
//! in this crate.
//!
//! Parsing untrusted serial input never returns these errors: a malformed
//! line from the radio is normal classroom noise, so `wire::parse_line`
//! simply yields `None`. `ProtoError` is reserved for values our own side
//! (server, browser bridge, frontend) asks us to build or send, where the
//! caller can correct the input.

/// A value that failed protocol validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtoError {
    /// A teacher command targeted a table outside `1..=9999`.
    ///
    /// Station *reports* may carry table `0` (an unset station), but a
    /// `T,<table>,<color>` command must name a real station ID.
    #[error("invalid table id {table}: teacher commands require 1..=9999")]
    InvalidTableId {
        /// The rejected table ID.
        table: u16,
    },

    /// A table range that does not satisfy `1 <= start <= end <= 9999`.
    #[error("invalid table range {start}..={end}: requires 1 <= start <= end <= 9999")]
    InvalidTableRange {
        /// The rejected first table ID.
        start: u16,
        /// The rejected last table ID.
        end: u16,
    },

    /// A color name other than `green`, `orange`, or `red`.
    #[error("invalid color {value:?}: expected \"green\", \"orange\", or \"red\"")]
    InvalidColor {
        /// The rejected color name, as supplied by the caller.
        value: String,
    },

    /// A session name that is empty, too long, or contains control
    /// characters. The reason is a fixed, human-readable explanation.
    #[error("invalid session name: {reason}")]
    InvalidSessionName {
        /// Why the name was rejected.
        reason: &'static str,
    },

    /// A viewer name that is empty, too long, contains control characters,
    /// or claims the creator's reserved name `host`. The reason is a fixed,
    /// human-readable explanation.
    #[error("invalid viewer name: {reason}")]
    InvalidViewerName {
        /// Why the name was rejected.
        reason: &'static str,
    },

    /// A serial line longer than the firmware's `MAX_LINE` budget.
    ///
    /// Nothing in this crate currently returns this variant (the command
    /// formatter only builds short lines by construction, and the line
    /// accumulator drops over-long input lines silently, mirroring the
    /// firmware); it exists so callers validating their own outbound lines
    /// against `wire::MAX_LINE_BYTES` can report the failure in the shared
    /// error type.
    #[error("serial line too long: {len} bytes exceeds maximum of {max} bytes")]
    LineTooLong {
        /// Length of the rejected line in bytes.
        len: usize,
        /// Maximum allowed length in bytes.
        max: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::ProtoError;

    #[test]
    fn error_messages_name_the_rejected_value() {
        assert_eq!(
            ProtoError::InvalidTableId { table: 0 }.to_string(),
            "invalid table id 0: teacher commands require 1..=9999"
        );
        assert_eq!(
            ProtoError::InvalidTableRange { start: 9, end: 2 }.to_string(),
            "invalid table range 9..=2: requires 1 <= start <= end <= 9999"
        );
        assert_eq!(
            ProtoError::InvalidColor {
                value: "blue".to_string()
            }
            .to_string(),
            "invalid color \"blue\": expected \"green\", \"orange\", or \"red\""
        );
        assert_eq!(
            ProtoError::InvalidSessionName {
                reason: "must not be empty"
            }
            .to_string(),
            "invalid session name: must not be empty"
        );
        assert_eq!(
            ProtoError::InvalidViewerName {
                reason: "must not be empty"
            }
            .to_string(),
            "invalid viewer name: must not be empty"
        );
        assert_eq!(
            ProtoError::LineTooLong { len: 65, max: 64 }.to_string(),
            "serial line too long: 65 bytes exceeds maximum of 64 bytes"
        );
    }

    #[test]
    fn errors_compare_by_value() {
        assert_eq!(
            ProtoError::InvalidTableId { table: 10_000 },
            ProtoError::InvalidTableId { table: 10_000 }
        );
        assert_ne!(
            ProtoError::InvalidTableId { table: 0 },
            ProtoError::InvalidTableRange { start: 0, end: 0 }
        );
    }
}
