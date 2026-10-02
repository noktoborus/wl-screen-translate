//! The clipboard that outlives the program.
//!
//! Under Wayland, as under X11, the program that copied serves the text to
//! whoever pastes it: once it quits, nothing is left to paste. A copy starts
//! the program again as [`SERVE_ARG`], in the background, to set the text and
//! serve it until something else is copied. arboard sets it through the
//! data-control protocol, or, where the compositor has none, such as GNOME,
//! through the X11 clipboard of XWayland, which the compositor shares with
//! Wayland.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use arboard::SetExtLinux;

/// The argument that starts the program as the server of a copied text,
/// read from its standard input.
pub const SERVE_ARG: &str = "--serve-clipboard";

/// Hands `text` to a new server of the clipboard.
pub fn copy(text: String) {
    let started = std::env::current_exe().and_then(|program| {
        Command::new(program)
            .arg(SERVE_ARG)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .current_dir("/")
            // Out of the group of the terminal, so its Ctrl+C spares it.
            .process_group(0)
            .spawn()
    });
    let mut child = match started {
        Ok(child) => child,
        Err(error) => return log::error!("clipboard server: {error}"),
    };
    // The server reads to the end: the text, then the closed pipe.
    if let Some(mut input) = child.stdin.take()
        && let Err(error) = input.write_all(text.as_bytes())
    {
        log::error!("clipboard server: {error}");
    }
    // Reaped once replaced, should the program still run.
    std::thread::spawn(move || child.wait());
}

/// Sets the text of the standard input as the clipboard and serves it until
/// something else is copied.
pub fn serve() -> Result<(), String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set().wait().text(text))
        .map_err(|error| error.to_string())
}
