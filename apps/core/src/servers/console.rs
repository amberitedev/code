//! Reading a server's console output line by line.

#![allow(dead_code)] // Wired in step 2: the shim writes and Core follows `console.log`.

use std::io::BufRead;

use tokio::sync::mpsc;

/// Forward each line of `reader` to `lines` on a thread of its own until the reader ends or the
/// receiver is dropped. Console output is untrusted text: a line that is not UTF-8 is decoded
/// lossily instead of ending the stream.
pub fn forward_lines<R>(reader: R, lines: mpsc::Sender<String>)
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(reader);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let text = String::from_utf8_lossy(&line);
            let text = text.trim_end_matches(['\n', '\r']).to_owned();
            if lines.blocking_send(text).is_err() {
                break;
            }
        }
    });
}
