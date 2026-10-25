//! A server's console output. The shim writes it to `runs/<server>/console.log` and Core reads it
//! from there. It is untrusted text: lines are capped at [`MAX_LINE`], decoded lossily, and the
//! log of a run is capped at [`MAX_LOG`].

use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::sync::mpsc;

/// The longest line kept. The rest of a longer line is dropped.
pub const MAX_LINE: usize = 64 * 1024;

/// When `console.log` grows past this, the shim starts it over and Core follows the new start.
pub const MAX_LOG: u64 = 32 * 1024 * 1024;

/// Starts the lines that answer Core's own health check (see `insight`). They stay in
/// `console.log` with this label and are left out of the console the App shows.
pub const HEALTH_CHECK: &str = "[Core health check] ";

/// Whether a console line is meant for people: not the answer to Core's health check.
pub fn for_people(line: &str) -> bool {
    !line.starts_with(HEALTH_CHECK)
}

/// How often a follower looks for new output.
const POLL: Duration = Duration::from_millis(250);

/// Append to `line` up to and including the next newline, keeping at most [`MAX_LINE`] bytes.
/// Returns how many bytes were consumed and whether a newline was reached; without one the input
/// ran out first and the line may continue later.
pub fn read_line(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
) -> io::Result<(usize, bool)> {
    let mut consumed = 0;
    loop {
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            return Ok((consumed, false));
        }
        let (taken, ended) = match available.iter().position(|&b| b == b'\n') {
            Some(newline) => (newline + 1, true),
            None => (available.len(), false),
        };
        let room = MAX_LINE.saturating_sub(line.len());
        line.extend_from_slice(&available[..taken.min(room)]);
        reader.consume(taken);
        consumed += taken;
        if ended {
            return Ok((consumed, true));
        }
    }
}

/// A raw console line as text, without its line ending.
pub fn decode(line: &[u8]) -> String {
    String::from_utf8_lossy(line)
        .trim_end_matches(['\n', '\r'])
        .to_owned()
}

/// The last `count` lines of a console log, and the offset where they end. A missing log has
/// no lines.
pub fn tail(path: &Path, count: usize) -> io::Result<(Vec<String>, u64)> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((Vec::new(), 0));
        }
        Err(error) => return Err(error),
    };
    let end = file.metadata()?.len();
    let window = (count * 512).max(MAX_LINE) as u64;
    let start = end.saturating_sub(window);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(end - start).read_to_end(&mut bytes)?;
    let mut lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
    // The text after the last newline is unfinished, and the first piece may be cut short.
    lines.pop();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let skip = lines.len().saturating_sub(count);
    let lines = lines[skip..]
        .iter()
        .map(|line| decode(&line[..line.len().min(MAX_LINE)]))
        .collect();
    let finished = end
        - bytes.rsplit(|&b| b == b'\n').next().map_or(0, <[u8]>::len) as u64;
    Ok((lines, finished))
}

/// Send each new line of the log at `path` from offset `from` to `lines`, on a thread of its
/// own, until the receiver is dropped. A log that shrinks was started over, by the shim's cap or
/// a new run, and is followed from its start.
pub fn follow(path: PathBuf, from: u64, lines: mpsc::Sender<String>) {
    std::thread::spawn(move || {
        let mut offset = from;
        let mut reader: Option<BufReader<File>> = None;
        let mut line = Vec::new();
        while !lines.is_closed() {
            if reader.is_none() {
                reader = File::open(&path).ok().and_then(|mut file| {
                    file.seek(SeekFrom::Start(offset)).ok()?;
                    Some(BufReader::new(file))
                });
            }
            if let Some(open) = &mut reader {
                match read_line(open, &mut line) {
                    Ok((consumed, true)) => {
                        offset += consumed as u64;
                        if lines.blocking_send(decode(&line)).is_err() {
                            return;
                        }
                        line.clear();
                        continue;
                    }
                    Ok((consumed, false)) => offset += consumed as u64,
                    Err(_) => reader = None,
                }
            }
            std::thread::sleep(POLL);
            let len = std::fs::metadata(&path).map_or(0, |meta| meta.len());
            if len < offset {
                offset = 0;
                line.clear();
                reader = None;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn long_lines_are_capped_and_bad_bytes_kept_as_text() {
        let mut input = vec![b'a'; MAX_LINE + 10];
        input.extend_from_slice(b"\nok \xff\r\nrest");
        let mut reader = io::Cursor::new(input);
        let mut line = Vec::new();

        assert_eq!(
            read_line(&mut reader, &mut line).unwrap(),
            (MAX_LINE + 11, true)
        );
        assert_eq!(line.len(), MAX_LINE);
        line.clear();
        read_line(&mut reader, &mut line).unwrap();
        assert_eq!(decode(&line), "ok \u{fffd}");
        line.clear();
        assert_eq!(read_line(&mut reader, &mut line).unwrap(), (4, false));
        assert_eq!(line, b"rest");
    }

    #[test]
    fn tail_skips_the_unfinished_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("console.log");
        std::fs::write(&path, "one\ntwo\nthree\nhalf").unwrap();
        let (lines, end) = tail(&path, 2).unwrap();
        assert_eq!(lines, ["two", "three"]);
        assert_eq!(end, 14);
    }

    #[tokio::test]
    async fn a_follower_sees_new_lines_and_a_restarted_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("console.log");
        std::fs::write(&path, "old\n").unwrap();
        let (sender, mut lines) = mpsc::channel(8);
        follow(path.clone(), 4, sender);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"new\n").unwrap();
        assert_eq!(lines.recv().await.unwrap(), "new");

        std::fs::write(&path, "again\n").unwrap();
        assert_eq!(lines.recv().await.unwrap(), "again");
    }
}
