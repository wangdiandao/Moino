//! L0 native platform seams: framed local IPC and menu capabilities; no internal dependencies.
#![forbid(unsafe_code)]

use interprocess::local_socket::{
    GenericNamespaced, Listener, ListenerNonblockingMode, ListenerOptions, Stream, prelude::*,
};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// Whether this platform uses an in-window menu overlay.
pub const fn has_window_menu() -> bool {
    !cfg!(target_os = "macos")
}

/// The prototype quit shortcut accepted by the pinned GPUI keymap parser.
pub const fn quit_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-q"
    } else {
        "ctrl-q"
    }
}

/// Bounded, versioned prototype launch request; no command or shell payloads.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchRequest {
    version: u32,
    paths: Vec<String>,
}

impl LaunchRequest {
    /// Builds a launch request for literal local paths.
    pub fn new(paths: Vec<String>) -> Self {
        Self { version: 1, paths }
    }
    /// Literal arguments to open in the first instance.
    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}

const MAX_FRAME: usize = 64 * 1024;

/// Creates a nonblocking listener; native backend uses pipes or local sockets.
pub fn listen(name: &str) -> io::Result<Listener> {
    ListenerOptions::new()
        .name(name.to_ns_name::<GenericNamespaced>()?)
        .nonblocking(ListenerNonblockingMode::Accept)
        .create_sync()
}

/// Connects a second instance to a literal endpoint.
pub fn connect(name: &str) -> io::Result<Stream> {
    Stream::connect(name.to_ns_name::<GenericNamespaced>()?)
}

/// Accepts a stream and explicitly clears inherited nonblocking mode on BSD systems.
/// Perform subsequent synchronous frame IO on a background executor, never the UI thread.
pub fn accept(listener: &Listener) -> io::Result<Stream> {
    let stream = listener.accept()?;
    stream.set_nonblocking(false)?;
    Ok(stream)
}

/// Sends one bounded request; execution never interprets its paths as commands.
pub fn send_request(stream: &mut impl Write, request: &LaunchRequest) -> io::Result<()> {
    let bytes = serde_json::to_vec(request)?;
    if bytes.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "IPC frame too large",
        ));
    }
    let size = u32::try_from(bytes.len()).map_err(io::Error::other)?;
    stream.write_all(&size.to_le_bytes())?;
    stream.write_all(&bytes)
}

/// Receives one frame, rejecting oversize lengths before allocating payload memory.
pub fn receive_request(stream: &mut impl Read) -> io::Result<LaunchRequest> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = usize::try_from(u32::from_le_bytes(length)).map_err(io::Error::other)?;
    if length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "IPC frame too large",
        ));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    let request: LaunchRequest = serde_json::from_slice(&bytes)?;
    if request.version != 1 || request.paths.iter().any(|path| path.contains('\0')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported launch request",
        ));
    }
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn framing_preserves_unicode_and_rejects_malformed_or_oversized_messages() {
        let request = LaunchRequest::new(vec!["文档/墨.md".into()]);
        let mut wire = Vec::new();
        send_request(&mut wire, &request).expect("valid request");
        assert_eq!(
            receive_request(&mut wire.as_slice()).expect("round trip"),
            request
        );
        assert!(receive_request(&mut u32::MAX.to_le_bytes().as_slice()).is_err());
        assert!(receive_request(&mut [1, 0, 0, 0, b'{'].as_slice()).is_err());
        let invalid = br#"{"version":2,"paths":[]}"#;
        let mut framed = u32::try_from(invalid.len())
            .expect("small")
            .to_le_bytes()
            .to_vec();
        framed.extend(invalid);
        assert!(receive_request(&mut framed.as_slice()).is_err());
    }
}
