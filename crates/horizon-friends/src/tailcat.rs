//! Tailcat tunnels: a second way to share a world, beside the invite codes of `code`/`invite`.
//!
//! [Tailcat for Minecraft](https://github.com/tailscale/tailcat-for-minecraft)'s helper
//! (`mclink-helper`, written in Go because Tailscale's userspace WireGuard, NAT traversal and DERP
//! relays are) carries the game's TCP stream between two loopback ports, as this crate's own tunnel
//! does. The game starts it as a child process and talks to it in lines of JSON on its standard
//! output; closing its standard input ends it.
//!
//! - Hosting: `mclink-helper host --target 127.0.0.1:<LAN port>` answers `ready` with an `mcl1_...`
//!   invitation.
//! - Joining: `mclink-helper join --invite mcl1_...` answers `ready` with a loopback `address` that
//!   forwards to the host's world.
//!
//! Nothing here depends on the helper's internals beyond that protocol. The helper is not shipped
//! with the game: see [`find_helper`] for where it is looked for.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader},
    net::SocketAddr,
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError, channel},
    time::{Duration, Instant},
};

/// What a Tailcat invitation starts with.
pub const INVITE_PREFIX: &str = "mcl1_";
/// The environment variable naming the helper's executable.
pub const HELPER_ENV: &str = "HORIZON_TAILCAT_HELPER";
/// How long a join waits for the helper to be ready (it finds a DERP region first).
pub const READY_TIMEOUT: Duration = Duration::from_secs(30);
/// A bound on an invitation's length: a real one is a few hundred characters.
const MAX_INVITE: usize = 4096;

/// Whether `code` is a Tailcat invitation. Only its shape is checked; the helper reads it.
pub fn is_invite(code: &str) -> bool {
    let code = code.trim();
    code.len() <= MAX_INVITE
        && code.strip_prefix(INVITE_PREFIX).is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

/// What tells one shared world from another, without the invitation: the first bytes of its hash.
/// (Native mods key a server's data by it, and an invitation is all a guest needs to join.)
pub fn public_part(code: &str) -> String {
    let digest = Sha256::digest(code.trim().as_bytes());
    let hex: String = digest[..6].iter().map(|b| format!("{b:02x}")).collect();
    format!("tailcat-{hex}")
}

/// A line the helper wrote.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Event {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub mode: String,
    /// `ready` in host mode: the invitation to give friends.
    #[serde(default)]
    pub invite: String,
    /// `ready` in join mode: the loopback address the game connects to.
    #[serde(default)]
    pub address: String,
    /// `error`: a short machine name (`derp_unreachable`, `invalid_invite`...).
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
}

impl Event {
    /// A line of the helper's output; None for anything that isn't an event.
    pub fn parse(line: &str) -> Option<Event> {
        serde_json::from_str::<Event>(line.trim())
            .ok()
            .filter(|e| !e.kind.is_empty())
    }
}

/// Where the helper is: `HORIZON_TAILCAT_HELPER`, else `mclink-helper` beside the game's
/// executable, else in a `tailcat` folder of the game folder.
pub fn find_helper() -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "mclink-helper.exe"
    } else {
        "mclink-helper"
    };
    if let Some(path) = std::env::var_os(HELPER_ENV).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "{HELPER_ENV} names {}, which isn't a file",
                path.display()
            ))
        };
    }
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)));
    let in_game_folder = std::env::current_dir()
        .ok()
        .map(|dir| dir.join("tailcat").join(name));
    beside_exe
        .into_iter()
        .chain(in_game_folder)
        .find(|p| p.is_file())
        .ok_or_else(|| {
            format!("the Tailcat helper ({name}) wasn't found: set {HELPER_ENV} or put it beside the game")
        })
}

/// A running helper. Dropping it ends it.
pub struct Helper {
    child: Child,
    /// Held open: the helper stops when its standard input closes.
    stdin: Option<ChildStdin>,
    events: Receiver<Event>,
    /// The last error the helper reported.
    pub error: Option<String>,
    /// It said `stopped`, or its output ended.
    pub stopped: bool,
}

impl Helper {
    /// Start the helper at `path` with `args`.
    pub fn spawn_at(path: &std::path::Path, args: &[&str]) -> Result<Helper, String> {
        let mut command = Command::new(path);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Its log lines (the helper's own, tailscale's) are not the game's.
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console flashes up beside the game.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("couldn't start the Tailcat helper {}: {e}", path.display()))?;
        let stdout = child.stdout.take().ok_or("the helper has no output")?;
        let stdin = child.stdin.take();
        let (send, events) = channel();
        std::thread::Builder::new()
            .name("horizon-tailcat-events".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Some(event) = Event::parse(&line)
                        && send.send(event).is_err()
                    {
                        break;
                    }
                }
                // The output ended: the helper did.
                let _ = send.send(Event {
                    kind: "stopped".into(),
                    ..Event::default()
                });
            })
            .map_err(|e| e.to_string())?;
        Ok(Helper {
            child,
            stdin,
            events,
            error: None,
            stopped: false,
        })
    }

    /// Start the helper wherever [`find_helper`] finds it.
    pub fn spawn(args: &[&str]) -> Result<Helper, String> {
        Helper::spawn_at(&find_helper()?, args)
    }

    /// Share the world on `lan` (a loopback address): the helper answers `ready` with the invitation.
    pub fn host(lan: SocketAddr) -> Result<Helper, String> {
        Helper::spawn(&["host", "--target", &lan.to_string()])
    }

    /// Join the world of `invite`: the helper answers `ready` with the loopback address.
    pub fn join(invite: &str) -> Result<Helper, String> {
        Helper::spawn(&["join", "--invite", invite.trim()])
    }

    fn note(&mut self, event: &Event) {
        match event.kind.as_str() {
            "error" => self.error = Some(describe_error(event)),
            "stopped" => self.stopped = true,
            _ => {}
        }
    }

    /// The events since the last call, without waiting.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    self.note(&event);
                    out.push(event);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stopped = true;
                    break;
                }
            }
        }
        out
    }

    /// Wait for `ready`, up to `timeout`. An error or an end before it is the answer's `Err`.
    pub fn wait_ready(&mut self, timeout: Duration) -> Result<Event, String> {
        let end = Instant::now() + timeout;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(event) => {
                    self.note(&event);
                    match event.kind.as_str() {
                        "ready" => return Ok(event),
                        "stopped" => {
                            return Err(self
                                .error
                                .clone()
                                .unwrap_or_else(|| "the Tailcat helper stopped".to_owned()));
                        }
                        _ => {}
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err("the Tailcat helper took too long to be ready".to_owned());
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(self
                        .error
                        .clone()
                        .unwrap_or_else(|| "the Tailcat helper stopped".to_owned()));
                }
            }
        }
    }

    /// End the helper: close its input, which it takes as the order to stop, then make sure.
    pub fn stop(&mut self) {
        drop(self.stdin.take());
        let end = Instant::now() + Duration::from_millis(500);
        while Instant::now() < end {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.stop();
    }
}

/// An `error` event as text for the player.
fn describe_error(event: &Event) -> String {
    match (event.code.as_str(), event.message.as_str()) {
        ("", "") => "the Tailcat helper failed".to_owned(),
        (code, "") => code.to_owned(),
        ("", message) => message.to_owned(),
        (code, message) => format!("{message} ({code})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitations_are_told_by_their_prefix_and_alphabet() {
        assert!(is_invite("mcl1_eyJ2ZXJzaW9uIjoxfQ"));
        assert!(is_invite("  mcl1_abc-DEF_123\n"));
        assert!(!is_invite("mcl1_"));
        assert!(!is_invite("mcl2_abc"));
        assert!(!is_invite("K7QM-3XPD"));
        assert!(!is_invite("mcl1_abc def"));
        assert!(!is_invite(&format!("mcl1_{}", "a".repeat(MAX_INVITE))));
    }

    #[test]
    fn the_public_part_shows_nothing_of_the_invitation() {
        let invite = "mcl1_eyJ2ZXJzaW9uIjoxfQ";
        let shown = public_part(invite);
        assert!(shown.starts_with("tailcat-"));
        assert!(!shown.contains("eyJ2"));
        assert_eq!(shown, public_part(&format!(" {invite}\n")));
        assert_ne!(shown, public_part("mcl1_other"));
    }

    #[test]
    fn the_helpers_lines_are_read() {
        let ready = Event::parse(r#"{"type":"ready","mode":"host","invite":"mcl1_x"}"#).unwrap();
        assert_eq!(ready.kind, "ready");
        assert_eq!(ready.invite, "mcl1_x");
        let join = Event::parse(r#"{"type":"ready","mode":"join","address":"127.0.0.1:41000"}"#);
        assert_eq!(join.unwrap().address, "127.0.0.1:41000");
        let error = Event::parse(r#"{"type":"error","code":"invalid_invite","message":"bad"}"#);
        assert_eq!(describe_error(&error.unwrap()), "bad (invalid_invite)");
        assert_eq!(Event::parse("2026/11/16 a log line"), None);
        assert_eq!(Event::parse("{}"), None);
    }

    /// A shell script stands in for the helper: it speaks the protocol and stops when its input
    /// closes, as the real one does.
    #[cfg(unix)]
    #[test]
    fn a_helper_is_started_waited_for_and_stopped() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("horizon-tailcat-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-helper");
        std::fs::write(
            &script,
            "#!/bin/sh\necho 'a log line'\necho '{\"type\":\"ready\",\"mode\":\"join\",\"address\":\"127.0.0.1:4242\"}'\ncat >/dev/null\necho '{\"type\":\"stopped\"}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut helper = Helper::spawn_at(&script, &[]).unwrap();
        let ready = helper.wait_ready(Duration::from_secs(10)).unwrap();
        assert_eq!(ready.address, "127.0.0.1:4242");
        helper.stop();
        // Its input closed, so it said `stopped` and ended on its own.
        assert!(matches!(helper.child.try_wait(), Ok(Some(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn an_error_before_ready_is_the_answer() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("horizon-tailcat-err-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("failing-helper");
        std::fs::write(
            &script,
            "#!/bin/sh\necho '{\"type\":\"error\",\"code\":\"derp_unreachable\",\"message\":\"no DERP\"}'\necho '{\"type\":\"stopped\"}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut helper = Helper::spawn_at(&script, &[]).unwrap();
        let err = helper.wait_ready(Duration::from_secs(10)).unwrap_err();
        assert_eq!(err, "no DERP (derp_unreachable)");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
