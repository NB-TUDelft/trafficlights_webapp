// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! `rust-embed` needs `web/dist` to exist when the server compiles. When the
//! frontend has not been built yet (a fresh clone, or `cargo test` before
//! `nub --cwd web run build`), leave a placeholder page so the crate still
//! builds. A real frontend build replaces it.

use std::path::PathBuf;
use std::{env, fs};

const PLACEHOLDER: &str = concat!(
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">",
    "<title>Classroom Traffic Lights</title></head>",
    "<body><p>Frontend not built yet. Run: nub --cwd web run build</p></body></html>",
);

fn main() {
    let manifest = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR for build scripts"),
    );
    let dist = manifest.join("../../web/dist");
    let index = dist.join("index.html");
    if !index.exists() {
        fs::create_dir_all(&dist).expect("create web/dist for rust-embed");
        fs::write(&index, PLACEHOLDER).expect("write placeholder web/dist/index.html");
    }
    println!("cargo:rerun-if-changed=../../web/dist/index.html");
}
