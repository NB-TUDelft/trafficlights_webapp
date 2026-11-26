// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runtime configuration, read from environment variables in production and
//! built directly (usually from [`ServerConfig::default`]) in tests.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

/// Default `BIND`: loopback only, so a bare local run is never exposed.
pub const DEFAULT_BIND: &str = "127.0.0.1:3000";
/// Default `DISCONNECT_GRACE_SECONDS` (5 minutes).
pub const DEFAULT_DISCONNECT_GRACE_SECONDS: u64 = 300;
/// Default `LOG_DIR` for the session statistics `.jsonl` files.
pub const DEFAULT_LOG_DIR: &str = "/tmp/trafficlights-logs";
/// Default `LOG_RETENTION_SECONDS` after `session_end` (10 minutes).
pub const DEFAULT_LOG_RETENTION_SECONDS: u64 = 600;
/// Default `MAX_SESSIONS`.
pub const DEFAULT_MAX_SESSIONS: usize = 32;
/// Default `MAX_CONNECTIONS`.
pub const DEFAULT_MAX_CONNECTIONS: usize = 512;
/// Default [`ServerConfig::auth_window`].
pub const DEFAULT_AUTH_WINDOW: Duration = Duration::from_secs(1);

/// A configuration variable that could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// The variable is set but is not a valid value.
    #[error("{name} must be {expected}, got {value:?}")]
    Invalid {
        /// The environment variable name.
        name: &'static str,
        /// What a valid value looks like.
        expected: &'static str,
        /// The rejected value.
        value: String,
    },
}

/// Everything the server needs to know at startup.
///
/// `Debug` redacts the creation PIN so the configuration can be traced
/// safely.
#[derive(Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// `BIND`: the socket address to listen on.
    pub bind: String,
    /// `PUBLIC_ORIGIN`: exact allowed `Origin` values for WebSocket
    /// upgrades, lowercased. Empty (the variable unset) disables the check
    /// for local development; a set variable never parses to empty.
    pub public_origins: Vec<String>,
    /// `CREATE_SESSION_PIN`: required to create a session, when set.
    pub create_session_pin: Option<String>,
    /// `DISCONNECT_GRACE_SECONDS`: how long a session survives without a
    /// connected receiver before it ends with `grace_timeout`.
    pub disconnect_grace: Duration,
    /// `LOG_DIR`: where the statistics `.jsonl` files are written.
    pub log_dir: PathBuf,
    /// `LOG_RETENTION_SECONDS`: how long an ended session's log stays
    /// downloadable before it is deleted.
    pub log_retention: Duration,
    /// `MAX_SESSIONS`: the cap on simultaneously active sessions.
    pub max_sessions: usize,
    /// `MAX_CONNECTIONS`: the cap on simultaneously open WebSocket
    /// connections (lobby and sessions together).
    pub max_connections: usize,
    /// `CUSTOM_SCRIPT_SRC`: an optional custom script added to the page
    /// head.
    pub custom_script: Option<CustomScript>,
    /// How long a new session connection may take to send its `auth`
    /// message before it is counted as a viewer. The creator's browser
    /// authenticates right after connecting, so it never shows up as a
    /// passing viewer in the lobby or in the `viewer_count` statistics.
    /// Not read from the environment; tests shorten it.
    pub auth_window: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND.to_string(),
            public_origins: Vec::new(),
            create_session_pin: None,
            disconnect_grace: Duration::from_secs(DEFAULT_DISCONNECT_GRACE_SECONDS),
            log_dir: PathBuf::from(DEFAULT_LOG_DIR),
            log_retention: Duration::from_secs(DEFAULT_LOG_RETENTION_SECONDS),
            max_sessions: DEFAULT_MAX_SESSIONS,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            custom_script: None,
            auth_window: DEFAULT_AUTH_WINDOW,
        }
    }
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerConfig")
            .field("bind", &self.bind)
            .field("public_origins", &self.public_origins)
            .field(
                "create_session_pin",
                &self.create_session_pin.as_ref().map(|_| "<redacted>"),
            )
            .field("disconnect_grace", &self.disconnect_grace)
            .field("log_dir", &self.log_dir)
            .field("log_retention", &self.log_retention)
            .field("max_sessions", &self.max_sessions)
            .field("max_connections", &self.max_connections)
            .field("custom_script", &self.custom_script)
            .field("auth_window", &self.auth_window)
            .finish()
    }
}

impl ServerConfig {
    /// Read the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Read the configuration through `lookup` (an environment stand-in).
    /// Unset or blank variables take their defaults.
    ///
    /// `DISCONNECT_GRACE_SECONDS` must be at least 1: a zero grace period
    /// would end every session the moment it is created, before the
    /// creator could attach the receiver. `LOG_RETENTION_SECONDS` may be
    /// `0` (delete the log as soon as the session ends).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let defaults = Self::default();
        Ok(Self {
            bind: get("BIND").unwrap_or(defaults.bind),
            public_origins: match get("PUBLIC_ORIGIN") {
                Some(value) => parse_origins(&value)?,
                None => Vec::new(),
            },
            create_session_pin: get("CREATE_SESSION_PIN"),
            disconnect_grace: match get("DISCONNECT_GRACE_SECONDS") {
                Some(value) => {
                    Duration::from_secs(parse_number("DISCONNECT_GRACE_SECONDS", &value, 1)?)
                }
                None => defaults.disconnect_grace,
            },
            log_dir: get("LOG_DIR").map_or(defaults.log_dir, PathBuf::from),
            log_retention: match get("LOG_RETENTION_SECONDS") {
                Some(value) => {
                    Duration::from_secs(parse_number("LOG_RETENTION_SECONDS", &value, 0)?)
                }
                None => defaults.log_retention,
            },
            max_sessions: match get("MAX_SESSIONS") {
                Some(value) => {
                    let max = parse_number("MAX_SESSIONS", &value, 1)?;
                    usize::try_from(max).unwrap_or(usize::MAX)
                }
                None => defaults.max_sessions,
            },
            max_connections: match get("MAX_CONNECTIONS") {
                Some(value) => {
                    let max = parse_number("MAX_CONNECTIONS", &value, 1)?;
                    usize::try_from(max).unwrap_or(usize::MAX)
                }
                None => defaults.max_connections,
            },
            custom_script: get("CUSTOM_SCRIPT_SRC")
                .map(|value| CustomScript::parse(&value))
                .transpose()?,
            auth_window: defaults.auth_window,
        })
    }

    /// Whether a WebSocket upgrade with this `Origin` header may proceed:
    /// always when `PUBLIC_ORIGIN` is unset, otherwise only on an exact
    /// match with one of the configured origins.
    pub fn origin_allowed(&self, origin: Option<&str>) -> bool {
        if self.public_origins.is_empty() {
            return true;
        }
        origin.is_some_and(|origin| self.public_origins.iter().any(|allowed| allowed == origin))
    }
}

/// `CUSTOM_SCRIPT_SRC`: the address of a custom script that every page
/// loads from its head (`<script src="..." defer>`), and the origin the
/// page's Content-Security-Policy allows for it, derived from the address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomScript {
    src: String,
    origin: String,
}

impl CustomScript {
    /// Parse a `CUSTOM_SCRIPT_SRC` value: an absolute `https://` or
    /// `http://` URL of printable ASCII without quotes, angle brackets,
    /// backslashes, or backticks, whose host is a plain DNS name or IPv4
    /// address with an optional port (no user info). Anything else is a
    /// configuration error, never a page with a broken or unsafe tag.
    pub fn parse(value: &str) -> Result<Self, ConfigError> {
        let invalid = || ConfigError::Invalid {
            name: "CUSTOM_SCRIPT_SRC",
            expected: "an absolute http(s) URL such as https://scripts.example.com/script.js",
            value: value.to_string(),
        };
        let src = value.trim();
        let printable = src.bytes().all(|byte| {
            byte.is_ascii_graphic() && !matches!(byte, b'"' | b'\'' | b'<' | b'>' | b'\\' | b'`')
        });
        if !printable {
            return Err(invalid());
        }
        let lower = src.to_ascii_lowercase();
        let scheme = ["https://", "http://"]
            .into_iter()
            .find(|scheme| lower.starts_with(scheme))
            .ok_or_else(invalid)?;
        let rest = &lower[scheme.len()..];
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if !valid_authority(authority) {
            return Err(invalid());
        }
        Ok(Self {
            src: src.to_string(),
            origin: format!("{scheme}{authority}"),
        })
    }

    /// The script's URL, as configured.
    pub fn src(&self) -> &str {
        &self.src
    }

    /// The script's origin (`scheme://host[:port]`, lowercased), which the
    /// page's Content-Security-Policy allows.
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// Whether `authority` is `host[:port]`: a host of letters, digits, `-`,
/// and `.` not starting with `-` or `.`, and an optional 1 to 5 digit port.
fn valid_authority(authority: &str) -> bool {
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let host_ok = !host.is_empty()
        && !host.starts_with(['-', '.'])
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'));
    let port_ok = port.is_none_or(|port| {
        (1..=5).contains(&port.len()) && port.bytes().all(|byte| byte.is_ascii_digit())
    });
    host_ok && port_ok
}

/// Whether a `BIND` value names a loopback host (`127.0.0.1`, `::1`, or
/// `localhost`), so a bare local run needs no `PUBLIC_ORIGIN`.
pub fn is_loopback_bind(bind: &str) -> bool {
    let host = if let Some(rest) = bind.trim().strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        bind.trim()
            .rsplit_once(':')
            .map_or(bind.trim(), |(host, _)| host)
    };
    host.eq_ignore_ascii_case("127.0.0.1")
        || host.eq_ignore_ascii_case("::1")
        || host.eq_ignore_ascii_case("localhost")
}

/// Split a comma-separated `PUBLIC_ORIGIN` into the exact strings browsers
/// send: `http://` or `https://`, a host, an optional port, nothing else,
/// lowercased. A trailing slash is dropped, but a path, query, fragment,
/// user info, or missing scheme is a configuration error, and so is a value
/// with no origin in it at all: a typo must never switch the check off.
fn parse_origins(value: &str) -> Result<Vec<String>, ConfigError> {
    let invalid = || ConfigError::Invalid {
        name: "PUBLIC_ORIGIN",
        expected: "comma-separated origins such as https://lights.example.com",
        value: value.to_string(),
    };
    let mut origins = Vec::new();
    for origin in value.split(',') {
        let origin = origin.trim().trim_end_matches('/').to_ascii_lowercase();
        if origin.is_empty() {
            continue;
        }
        let authority = origin
            .strip_prefix("https://")
            .or_else(|| origin.strip_prefix("http://"))
            .ok_or_else(invalid)?;
        let malformed = authority.is_empty()
            || authority
                .chars()
                .any(|ch| matches!(ch, '/' | '?' | '#' | '@') || ch.is_whitespace());
        if malformed {
            return Err(invalid());
        }
        origins.push(origin);
    }
    if origins.is_empty() {
        return Err(invalid());
    }
    Ok(origins)
}

fn parse_number(name: &'static str, value: &str, min: u64) -> Result<u64, ConfigError> {
    let invalid = || ConfigError::Invalid {
        name,
        expected: if min == 0 {
            "a whole number"
        } else {
            "a whole number of at least 1"
        },
        value: value.to_string(),
    };
    let number: u64 = value.parse().map_err(|_| invalid())?;
    if number < min {
        return Err(invalid());
    }
    Ok(number)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use super::{is_loopback_bind, ConfigError, CustomScript, ServerConfig};

    fn config(vars: &[(&str, &str)]) -> Result<ServerConfig, ConfigError> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        ServerConfig::from_lookup(|name| vars.get(name).cloned())
    }

    #[test]
    fn unset_variables_take_the_documented_defaults() {
        let config = config(&[]);
        assert_eq!(config, Ok(ServerConfig::default()));
        let config = ServerConfig::default();
        assert_eq!(config.bind, "127.0.0.1:3000");
        assert!(config.public_origins.is_empty());
        assert_eq!(config.create_session_pin, None);
        assert_eq!(config.disconnect_grace, Duration::from_secs(300));
        assert_eq!(config.log_dir.to_str(), Some("/tmp/trafficlights-logs"));
        assert_eq!(config.log_retention, Duration::from_secs(600));
        assert_eq!(config.max_sessions, 32);
        assert_eq!(config.max_connections, 512);
        assert_eq!(config.auth_window, Duration::from_secs(1));
    }

    #[test]
    fn every_variable_is_read() {
        let config = config(&[
            ("BIND", "0.0.0.0:8080"),
            (
                "PUBLIC_ORIGIN",
                " https://lights.example.com/ , http://localhost:5173,",
            ),
            ("CREATE_SESSION_PIN", "4321"),
            ("DISCONNECT_GRACE_SECONDS", "5"),
            ("LOG_DIR", "/var/tmp/logs"),
            ("LOG_RETENTION_SECONDS", "0"),
            ("MAX_SESSIONS", "2"),
            ("MAX_CONNECTIONS", "7"),
            (
                "CUSTOM_SCRIPT_SRC",
                " https://Scripts.Example.com/s.js?site=A ",
            ),
        ]);
        let Ok(config) = config else {
            panic!("valid configuration rejected: {config:?}");
        };
        assert_eq!(config.bind, "0.0.0.0:8080");
        assert_eq!(
            config.public_origins,
            vec![
                "https://lights.example.com".to_string(),
                "http://localhost:5173".to_string()
            ]
        );
        assert_eq!(config.create_session_pin.as_deref(), Some("4321"));
        assert_eq!(config.disconnect_grace, Duration::from_secs(5));
        assert_eq!(config.log_dir.to_str(), Some("/var/tmp/logs"));
        assert_eq!(config.log_retention, Duration::ZERO);
        assert_eq!(config.max_sessions, 2);
        assert_eq!(config.max_connections, 7);
        let script = config.custom_script.expect("custom script");
        assert_eq!(script.src(), "https://Scripts.Example.com/s.js?site=A");
        assert_eq!(script.origin(), "https://scripts.example.com");
    }

    #[test]
    fn custom_script_src_gives_its_origin_and_rejects_unsafe_values() {
        for (src, origin) in [
            (
                "https://app.example.io/api/script.js?siteId=A1&x=2",
                "https://app.example.io",
            ),
            ("http://127.0.0.1:8080/s.js", "http://127.0.0.1:8080"),
            ("HTTPS://cdn.example.com", "https://cdn.example.com"),
            ("https://cdn.example.com#frag", "https://cdn.example.com"),
        ] {
            let parsed = CustomScript::parse(src);
            assert_eq!(
                parsed.as_ref().map(CustomScript::origin),
                Ok(origin),
                "{src}"
            );
            assert_eq!(parsed.as_ref().map(CustomScript::src), Ok(src), "{src}");
        }
        // Blank means unset.
        assert_eq!(
            config(&[("CUSTOM_SCRIPT_SRC", "  ")]).map(|c| c.custom_script),
            Ok(None)
        );
        for bad in [
            "script.js",
            "/script.js",
            "//cdn.example.com/s.js",
            "ftp://cdn.example.com/s.js",
            "javascript:alert(1)",
            "https://",
            "https:///s.js",
            "https://:8080/s.js",
            "https://.example.com/s.js",
            "https://user@cdn.example.com/s.js",
            "https://cdn.example.com:/s.js",
            "https://cdn.example.com:123456/s.js",
            "https://cdn.example.com:80a/s.js",
            "https://[::1]/s.js",
            "https://cdn.example.com/a b.js",
            "https://cdn.example.com/\"onload=\"x",
            "https://cdn.example.com/'x",
            "https://cdn.example.com/<x>",
            "https://cdn.example.com/a\\b",
            "https://cdn.example.com/`x`",
            "https://cdn.example.com/\u{e9}.js",
            "https://cdn.example.com/a\tb.js",
            "https://cdn;example.com/s.js",
        ] {
            assert!(
                matches!(
                    config(&[("CUSTOM_SCRIPT_SRC", bad)]),
                    Err(ConfigError::Invalid {
                        name: "CUSTOM_SCRIPT_SRC",
                        ..
                    })
                ),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn max_connections_must_be_at_least_one() {
        assert!(config(&[("MAX_CONNECTIONS", "0")]).is_err());
        assert!(config(&[("MAX_CONNECTIONS", "many")]).is_err());
    }

    #[test]
    fn loopback_binds_are_recognized() {
        for bind in [
            "127.0.0.1:3000",
            "[::1]:3000",
            "localhost:3000",
            "LOCALHOST:3000",
            "127.0.0.1",
        ] {
            assert!(is_loopback_bind(bind), "{bind}");
        }
        for bind in [
            "0.0.0.0:3000",
            "[::]:3000",
            "192.0.2.1:3000",
            "example.com:3000",
        ] {
            assert!(!is_loopback_bind(bind), "{bind}");
        }
    }

    #[test]
    fn public_origin_never_parses_to_an_open_check() {
        let parsed = config(&[(
            "PUBLIC_ORIGIN",
            "HTTPS://Lights.Example.com:8443/,http://localhost:5173",
        )]);
        assert_eq!(
            parsed.map(|c| c.public_origins),
            Ok(vec![
                "https://lights.example.com:8443".to_string(),
                "http://localhost:5173".to_string()
            ])
        );
        // Unset or blank is the development default: no check.
        assert_eq!(
            config(&[("PUBLIC_ORIGIN", "  ")]).map(|c| c.public_origins),
            Ok(Vec::new())
        );
        for bad in [
            ",",
            "/",
            " , / ,",
            "lights.example.com",
            "https://",
            "https://lights.example.com/app",
            "https://lights.example.com?x=1",
            "https://user@lights.example.com",
            "https://lights example.com",
            "ftp://lights.example.com",
            "https://ok.example.com,nope",
        ] {
            assert!(
                matches!(
                    config(&[("PUBLIC_ORIGIN", bad)]),
                    Err(ConfigError::Invalid {
                        name: "PUBLIC_ORIGIN",
                        ..
                    })
                ),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn blank_pin_disables_the_pin() {
        let config = config(&[("CREATE_SESSION_PIN", "   ")]);
        assert_eq!(config.map(|c| c.create_session_pin), Ok(None));
    }

    #[test]
    fn malformed_numbers_are_rejected() {
        assert!(config(&[("MAX_SESSIONS", "0")]).is_err());
        assert!(config(&[("MAX_SESSIONS", "lots")]).is_err());
        assert!(config(&[("DISCONNECT_GRACE_SECONDS", "-1")]).is_err());
        assert!(config(&[("DISCONNECT_GRACE_SECONDS", "0")]).is_err());
        assert!(config(&[("LOG_RETENTION_SECONDS", "1.5")]).is_err());
    }

    #[test]
    fn debug_output_redacts_the_pin() {
        let config = ServerConfig {
            create_session_pin: Some("secret-pin-9876".to_string()),
            ..ServerConfig::default()
        };
        let text = format!("{config:?}");
        assert!(!text.contains("secret-pin-9876"), "{text}");
        assert!(text.contains("<redacted>"), "{text}");
    }

    #[test]
    fn origin_check_is_exact_and_off_when_unconfigured() {
        let open = ServerConfig::default();
        assert!(open.origin_allowed(None));
        assert!(open.origin_allowed(Some("https://evil.example")));

        let locked = ServerConfig {
            public_origins: vec!["https://lights.example.com".to_string()],
            ..ServerConfig::default()
        };
        assert!(locked.origin_allowed(Some("https://lights.example.com")));
        assert!(!locked.origin_allowed(None));
        assert!(!locked.origin_allowed(Some("https://lights.example.com.evil.example")));
        assert!(!locked.origin_allowed(Some("http://lights.example.com")));
        assert!(!locked.origin_allowed(Some("")));
    }
}
