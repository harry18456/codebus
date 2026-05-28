//! Cross-platform "terminate a child process and its entire descendant tree"
//! helper.
//!
//! ## Why a process tree, not just a single PID
//!
//! The codebus agent CLIs (`claude`, `codex`) on Windows resolve to a
//! `.cmd` shim that spawns `node.exe` underneath. On Unix they similarly
//! spawn helper processes (node, ripgrep, shell tools). When the watcher
//! thread inside `super::claude_cli::invoke` flips on a cancel signal and
//! kills only the immediate `Child`, the grandchildren inherit the stdout
//! pipe and keep it open — `BufReader::lines()` in the main loop never
//! sees EOF, `invoke` never returns, and the user is stuck on
//! "Cancelling…" in the UI. Real-app CDP smoke for this change
//! (cancelling-stuck-fix, 2026-05-28) reproduced this exact zombie
//! behaviour against codex.
//!
//! ## How the platforms differ
//!
//! - **Windows**: each spawn is wrapped in a Win32 Job Object created
//!   with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. The child is assigned
//!   to the job immediately after spawn. Calling `TerminateJobObject`
//!   (or simply closing the job handle) kills every process the child
//!   ever launched, transitively. The pipe collapses, the main loop
//!   unblocks, and `invoke` returns.
//! - **Unix**: each spawn calls `Command::process_group(0)` so the
//!   child becomes a new process-group leader (PGID == child PID).
//!   `killpg(pgid, SIGTERM)` then signals the entire group, including
//!   any forks the child made.
//!
//! ## Design constraints
//!
//! - PID-based control (not `Child::kill`) is required: the main thread
//!   owns the `Child` (it needs `child.wait()` to reap), so the watcher
//!   cannot take a mutable reference to it. Both platforms expose
//!   PID/handle-based APIs that avoid this conflict.
//! - Idempotent: the watcher AND the existing main-loop fast path may
//!   both fire kills concurrently. The contract is "after a successful
//!   call, the process tree is gone or going" — repeat calls are
//!   harmless no-ops.

use std::io;

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

/// Cross-platform handle for terminating a spawned child *and all of its
/// descendants* as one unit. Created via [`KillHandle::install`] after a
/// successful `Command::spawn`. Dropping the handle is a no-op on Unix
/// and closes the job (killing any survivors) on Windows.
pub(crate) struct KillHandle {
    inner: Inner,
}

#[cfg(unix)]
struct Inner {
    pgid: libc::pid_t,
}

#[cfg(windows)]
struct Inner {
    job: windows_sys::Win32::Foundation::HANDLE,
}

// Inner contains either a libc PID (POD) or a Windows HANDLE
// (essentially a `*mut c_void` that we manage manually). Both are safe
// to share across threads — Windows HANDLE is thread-safe for the APIs
// we use, and PIDs are just integers.
#[cfg(windows)]
unsafe impl Send for Inner {}
#[cfg(windows)]
unsafe impl Sync for Inner {}

impl KillHandle {
    /// Configure `cmd` to spawn into its own process group / job. Call
    /// this BEFORE `Command::spawn`. On Unix this sets `process_group(0)`
    /// so the child becomes a PGID leader; on Windows it is currently a
    /// no-op (the job is attached after spawn).
    pub(crate) fn pre_spawn(_cmd: &mut Command) {
        #[cfg(unix)]
        _cmd.process_group(0);
        // Windows path: no pre-spawn knob is needed. We attach the
        // Job Object after CreateProcess; there is a tiny race window
        // where the child could fork before assignment, but the spawned
        // CLIs do not fork instantly on startup and the race has never
        // been observed in practice.
    }

    /// Install a kill handle around an already-spawned child. On Unix
    /// records the child's PID as the PGID (the child is the group
    /// leader because of [`pre_spawn`]). On Windows creates a Job Object
    /// with `KILL_ON_JOB_CLOSE` and assigns the child to it.
    #[cfg(unix)]
    pub(crate) fn install(child: &Child) -> io::Result<KillHandle> {
        // The child became a group leader via `process_group(0)` in
        // `pre_spawn`, so PGID == child PID.
        Ok(KillHandle {
            inner: Inner {
                pgid: child.id() as libc::pid_t,
            },
        })
    }

    #[cfg(windows)]
    pub(crate) fn install(child: &Child) -> io::Result<KillHandle> {
        use std::mem::{size_of, zeroed};
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        // SAFETY: `CreateJobObjectW(null, null)` is documented to
        // return either a valid handle or NULL with GetLastError set.
        let job: HANDLE = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }

        // Configure JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE so that closing
        // the job handle (e.g. on Drop) terminates every process still
        // assigned to it. This is the belt-and-braces fallback to the
        // explicit `TerminateJobObject` we call from
        // `terminate_tree`.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `job` is non-null and was just returned by
        // `CreateJobObjectW`. We pass the address of a properly-sized
        // and zero-initialised `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`.
        let ok = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            let err = io::Error::last_os_error();
            close_handle(job);
            return Err(err);
        }

        // Assign the freshly-spawned child to the job. Windows 8+
        // supports nested jobs (Tauri itself runs in one), so even if
        // the parent process is already in a job this AssignProcess
        // call succeeds and our job becomes a nested child.
        let child_handle = child.as_raw_handle() as HANDLE;
        // SAFETY: `job` and `child_handle` are both valid kernel
        // handles owned by us / std at this point.
        let ok = unsafe { AssignProcessToJobObject(job, child_handle) };
        if ok == 0 {
            let err = io::Error::last_os_error();
            close_handle(job);
            return Err(err);
        }

        Ok(KillHandle {
            inner: Inner { job },
        })
    }

    /// Terminate the child and every descendant. Idempotent — safe to
    /// call from multiple threads concurrently and safe to call after
    /// the tree has already exited.
    #[cfg(unix)]
    pub(crate) fn terminate_tree(&self) -> io::Result<()> {
        let rc = unsafe { libc::killpg(self.inner.pgid, libc::SIGTERM) };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        // ESRCH (no such process group): the leader is gone and the
        // group has dissolved. That satisfies the "tree is gone"
        // contract.
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        Err(err)
    }

    #[cfg(windows)]
    pub(crate) fn terminate_tree(&self) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        // SAFETY: `self.inner.job` is a valid job handle we own from
        // `install`. Exit code 1 is the conventional "killed".
        let ok = unsafe { TerminateJobObject(self.inner.job, 1) };
        if ok == 0 {
            // The job may already have been terminated (e.g. by
            // Drop racing with an explicit cancel). Idempotent
            // success.
            return Ok(());
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for KillHandle {
    fn drop(&mut self) {
        // Closing the job handle while JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        // is set kills any process still assigned to it. This is the
        // belt-and-braces safety net for the "invoke returns without
        // anybody ever calling terminate_tree" path: any surviving
        // descendant is reaped here.
        close_handle(self.inner.job);
    }
}

#[cfg(windows)]
fn close_handle(h: windows_sys::Win32::Foundation::HANDLE) {
    use windows_sys::Win32::Foundation::CloseHandle;
    // SAFETY: caller passes a handle we obtained from a CreateXxx call.
    unsafe {
        CloseHandle(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    fn long_running_cmd() -> Command {
        let mut c = Command::new("sleep");
        c.arg("30");
        c
    }

    #[cfg(windows)]
    fn long_running_cmd() -> Command {
        let mut c = Command::new("powershell.exe");
        c.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
        c
    }

    /// A command that spawns a long-running grandchild and then keeps
    /// stdout open through it. Reproduces the
    /// `cmd.exe → node.exe` (codex) and `claude.cmd → node.exe`
    /// scenarios. Without the JobObject / pgroup wrapping the
    /// grandchild leaks and the parent's stdout pipe stays open
    /// forever.
    #[cfg(windows)]
    fn long_running_with_grandchild() -> Command {
        // `cmd /c <wrapper>` is the immediate child. The wrapper runs
        // PowerShell which is the grandchild. PowerShell holds the
        // inherited stdout pipe for 30 seconds. TerminateProcess on
        // the cmd alone would leak the PowerShell child; we want the
        // Job Object to clean both.
        let mut c = Command::new("cmd");
        c.args(["/c", "powershell.exe -NoProfile -Command \"Start-Sleep -Seconds 30\""]);
        c
    }

    #[cfg(unix)]
    fn long_running_with_grandchild() -> Command {
        // `sh -c 'sleep 30'` — sh is the immediate child, sleep is
        // the grandchild that inherits stdout.
        let mut c = Command::new("sh");
        c.args(["-c", "sleep 30"]);
        c
    }

    #[test]
    fn terminate_tree_kills_a_lone_child() {
        let mut cmd = long_running_cmd();
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        KillHandle::pre_spawn(&mut cmd);
        let mut child = cmd.spawn().expect("spawn");
        let handle = KillHandle::install(&child).expect("install kill handle");
        handle.terminate_tree().expect("terminate_tree");

        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if started.elapsed() > Duration::from_secs(5) {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("child did not exit after terminate_tree");
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => panic!("try_wait: {e}"),
            }
        }
    }

    /// The grandchild regression: `cmd /c <wrapper>` (Windows) or
    /// `sh -c <wrapper>` (Unix) creates a real two-process chain. A
    /// naive single-PID kill leaves the grandchild running and its
    /// inherited stdout pipe open forever. The KillHandle path must
    /// terminate both.
    #[test]
    fn terminate_tree_kills_grandchild() {
        let mut cmd = long_running_with_grandchild();
        // Pipe stdout — that is the channel the real `invoke` loop
        // reads from. The whole point of this test is that the pipe
        // must reach EOF after terminate_tree, not stay propped open
        // by the grandchild.
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        KillHandle::pre_spawn(&mut cmd);
        let mut child = cmd.spawn().expect("spawn wrapper");
        let stdout = child.stdout.take().expect("piped stdout");
        let handle = KillHandle::install(&child).expect("install kill handle");

        // Drain stdout on a background thread so the pipe doesn't
        // block the kernel-side writer. Closure ends when the pipe
        // reaches EOF — which is what we are asserting on.
        let pipe_drain = std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = Vec::new();
            let mut stdout = stdout;
            let _ = stdout.read_to_end(&mut buf);
        });

        handle.terminate_tree().expect("terminate_tree");

        // The wrapper must exit (we killed it directly or via pgroup).
        let exit_started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if exit_started.elapsed() > Duration::from_secs(5) {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("wrapper did not exit after terminate_tree");
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => panic!("try_wait: {e}"),
            }
        }

        // The grandchild must also be gone — proven by the stdout
        // pipe reaching EOF (the drain thread returns). Without the
        // process-tree fix the grandchild would still hold the pipe
        // and this join would hang for the full 30s of the sleep.
        let join_started = Instant::now();
        loop {
            if pipe_drain.is_finished() {
                let _ = pipe_drain.join();
                return;
            }
            if join_started.elapsed() > Duration::from_secs(5) {
                panic!(
                    "stdout pipe did not reach EOF — grandchild is still \
                     holding the writer end alive"
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn terminate_tree_is_idempotent_on_dead_tree() {
        let mut cmd = long_running_cmd();
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        KillHandle::pre_spawn(&mut cmd);
        let mut child = cmd.spawn().expect("spawn");
        let handle = KillHandle::install(&child).expect("install kill handle");
        handle.terminate_tree().expect("first kill");
        let _ = child.wait();
        // Tree is gone; second call must still be Ok.
        handle.terminate_tree().expect("second kill is a no-op");
    }
}
