// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! End-to-end tests of the trafficlights-webapp server: real axum routers on
//! ephemeral loopback ports, a minimal RFC 6455 client (see [`support`]),
//! temporary log directories, and the statistics `.jsonl` files on disk.
//!
//! - [`lobby`]: lobby snapshot and updates, session creation, the optional
//!   `CREATE_SESSION_PIN`, and the `MAX_SESSIONS` cap.
//! - [`roles`]: viewer versus creator permissions, the atomic offline check,
//!   creator-only serial commands and unlisted tables, creator reconnects.
//! - [`lifecycle`]: grace timeout, manual end, log retention, and graceful
//!   shutdown (in process and through the real binary on `SIGTERM`).
//! - [`http`]: `PUBLIC_ORIGIN` enforcement, malformed traffic, the
//!   creator-authorized `.jsonl` download with its full round trip, and the
//!   optional custom script in the page head.
//! - [`names`]: sessions that collect viewer names: who is logged for each
//!   change, unique names, and the reconnect takeover.

mod http;
mod lifecycle;
mod lobby;
mod names;
mod roles;
mod support;
