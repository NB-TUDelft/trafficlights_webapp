// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! Browser WebAssembly bridge for trafficlights-webapp.
//!
//! A thin `wasm-bindgen` wrapper over `trafficlights-proto`, loaded lazily by
//! the creator's browser only, so viewers never download it. Build it with
//! `scripts/build-wasm.sh`; the generated glue lands in `web/src/wasm/`.

use wasm_bindgen::prelude::wasm_bindgen;

/// Smoke export: the protocol version this module was built against. The
/// frontend can compare it with the server's to detect a stale cached module.
#[wasm_bindgen]
pub fn proto_version() -> u32 {
    trafficlights_proto::PROTO_VERSION
}

#[cfg(test)]
mod tests {
    use super::proto_version;

    #[test]
    fn exports_protocol_version_one() {
        assert_eq!(proto_version(), 1);
    }
}
