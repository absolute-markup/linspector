//! Bounded capture. Read-only children may time out; active mutations are never killed here.
use std::{
    io::{Read, Write},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const LIMIT: usize = 1024 * 1024;
#[derive(Debug)]
pub(crate) struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub truncated: bool,
    pub invalid_utf8: bool,
}
impl Output {
    pub fn success(&self) -> bool {
        self.code == Some(0) && !self.timed_out
    }
    pub fn query_text(self) -> Result<String, String> {
        if self.timed_out {
            return Err("Read-only command timed out".into());
        }
        if self.invalid_utf8 {
            return Err("Query returned non-UTF-8 output".into());
        }
        if self.truncated {
            return Err("Read-only command output exceeded its limit".into());
        }
        if !self.success() {
            return Err(format!("Command failed ({:?}): {}", self.code, self.stderr));
        }
        Ok(self.stdout)
    }
}
fn capture(mut reader: impl Read, limit: usize) -> std::io::Result<(String, bool, bool)> {
    let mut kept = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let keep = count.min(limit.saturating_sub(kept.len()));
        kept.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    let invalid_utf8 = std::str::from_utf8(&kept).is_err();
    Ok((
        String::from_utf8_lossy(&kept).into(),
        truncated,
        invalid_utf8,
    ))
}
pub(crate) fn run(command: &mut Command, deadline: Option<Duration>) -> Result<Output, String> {
    run_input(command, deadline, None)
}
pub(crate) fn run_input(
    command: &mut Command,
    deadline: Option<Duration>,
    input: Option<Vec<u8>>,
) -> Result<Output, String> {
    run_limited(command, deadline, input, LIMIT)
}
pub(crate) fn large_query(command: &mut Command) -> Result<Output, String> {
    run_limited(
        command,
        Some(Duration::from_secs(30)),
        None,
        128 * 1024 * 1024,
    )
}
fn run_limited(
    command: &mut Command,
    deadline: Option<Duration>,
    input: Option<Vec<u8>>,
    stdout_limit: usize,
) -> Result<Output, String> {
    command.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    // A separate group lets read-only query deadlines reap descendants holding pipes.
    // No deadline/group signal is used for authenticated mutations.
    if deadline.is_some() {
        command.process_group(0);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let writer = input.map(|bytes| {
        let mut pipe = child.stdin.take().unwrap();
        thread::spawn(move || pipe.write_all(&bytes))
    });
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || capture(stdout, stdout_limit));
    let err = thread::spawn(move || capture(stderr, LIMIT));
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        // Keep the leader unreaped while descendants hold pipes, so its process
        // group identity remains reserved until EOF or the query deadline.
        if out.is_finished() && err.is_finished() && writer.as_ref().is_none_or(|w| w.is_finished())
        {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => (),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(error.to_string());
                }
            }
        }
        if deadline.is_some_and(|limit| start.elapsed() >= limit) {
            timed_out = true;
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            // The unreaped child owns the isolated query group established above.
            unsafe {
                kill(-(child.id() as i32), 9);
            }
            let _ = child.kill();
            break child.wait().map_err(|error| error.to_string());
        }
        thread::sleep(Duration::from_millis(10));
    };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let (stdout, a, invalid_out) = out
        .join()
        .map_err(|_| "Output reader failed")?
        .map_err(|e| e.to_string())?;
    let (stderr, b, invalid_err) = err
        .join()
        .map_err(|_| "Error reader failed")?
        .map_err(|e| e.to_string())?;
    Ok(Output {
        code: status?.code(),
        stdout,
        stderr,
        timed_out,
        truncated: a || b,
        invalid_utf8: invalid_out || invalid_err,
    })
}
pub(crate) fn command(path: &str) -> Command {
    let mut command = Command::new(path);
    command
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LC_ALL", "C");
    command
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_capture_drains_and_marks_truncation() {
        let bytes = vec![b'x'; LIMIT + 9000];
        let (text, truncated, invalid) = capture(bytes.as_slice(), LIMIT).unwrap();
        assert_eq!(text.len(), LIMIT);
        assert!(truncated);
        assert!(!invalid);
        assert!(capture([0xff].as_slice(), LIMIT).unwrap().2);
    }
    #[test]
    fn query_deadline_reaps_the_direct_child() {
        let result = run(
            Command::new("/usr/bin/sleep").arg("10"),
            Some(Duration::from_millis(20)),
        )
        .unwrap();
        assert!(result.timed_out);
        assert!(!result.success());
    }
    #[test]
    fn child_failure_and_success_are_not_conflated() {
        assert!(!run(
            &mut Command::new("/usr/bin/false"),
            Some(Duration::from_secs(1))
        )
        .unwrap()
        .success());
        assert!(run(&mut Command::new("/usr/bin/true"), None)
            .unwrap()
            .success());
    }
}

#[cfg(test)]
mod pipe_tests {
    use super::*;
    #[test]
    fn request_pipe_is_closed_and_exit_codes_survive_transport() {
        let output = run_input(
            &mut Command::new("/usr/bin/cat"),
            None,
            Some(b"typed request".to_vec()),
        )
        .unwrap();
        assert!(output.success());
        assert_eq!(output.stdout, "typed request");
        for code in [126, 127] {
            // Controlled child failure, not a real authentication test.
            let output = run(
                Command::new("/usr/bin/sh").args(["-c", &format!("exit {code}")]),
                None,
            )
            .unwrap();
            assert_eq!(output.code, Some(code));
        }
    }
    #[test]
    fn query_deadline_reaps_descendants_holding_output_pipes() {
        for script in ["sleep 10 & wait", "sleep 10 &"] {
            let result = run(
                Command::new("/usr/bin/sh").args(["-c", script]),
                Some(Duration::from_millis(20)),
            )
            .unwrap();
            assert!(result.timed_out);
        }
    }
}
