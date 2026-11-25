// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! Shared, I/O-free protocol and domain types for trafficlights-webapp.
//!
//! This crate is compiled into both the Axum server (`trafficlights-server`)
//! and the browser WebAssembly module (`trafficlights-wasm`), so the two sides
//! can never disagree about the serial wire format or the structured events
//! exchanged over WebSocket. It performs no I/O.
//!
//! The scaffold only carries the protocol version; the domain types, line
//! framing, legacy and `!`-command parsers and the strict `T,<table>,<color>`
//! formatter described in "Architecture and design" arrive in the next slice.

/// Version of the structured event and command protocol shared by the server
/// and the browser module. Bump it when either side must reject the other.
pub const PROTO_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::PROTO_VERSION;

    #[test]
    fn protocol_version_starts_at_one() {
        assert_eq!(PROTO_VERSION, 1);
    }
}
