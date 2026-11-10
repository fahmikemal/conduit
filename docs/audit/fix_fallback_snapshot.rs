use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::{Arc, atomic::{AtomicBool, Ordering::Relaxed}};
static NO_OPENAT2: AtomicBool = AtomicBool::new(false);
fn open_beneath(root: &Path, rel: &[u8]) -> io::Result<File> {
    let rel = rel.strip_prefix(b"/").unwrap_or(rel);
    let rel: &[u8] = if rel.is_empty() { b"." } else { rel };
    let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY;
    if !NO_OPENAT2.load(Relaxed) {
        let dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
            .open(root)?;
        let c = CString::new(rel).map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
        // SAFETY: open_how is plain data; zero is a valid value for every field.
        let mut how: libc::open_how = unsafe { std::mem::zeroed() };
        how.flags = flags as u64;
        how.resolve = libc::RESOLVE_BENEATH | libc::RESOLVE_NO_MAGICLINKS;
        // SAFETY: valid directory fd, NUL-terminated path, and a correctly sized open_how.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                dir.as_raw_fd(),
                c.as_ptr(),
                &how as *const libc::open_how,
                std::mem::size_of::<libc::open_how>(),
            )
        };
        if fd >= 0 {
            // SAFETY: the kernel just returned this descriptor to us.
            return Ok(unsafe { File::from_raw_fd(fd as RawFd) });
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error() {
            Some(libc::ENOSYS | libc::EPERM) => {
                log::warn!(
                    "files: openat2 unavailable ({e}); using descriptor traversal without symlinks"
                );
                NO_OPENAT2.store(true, Relaxed);
            }
            // The path leads out of the root (a symlink); as if it did not exist.
            Some(libc::EXDEV) => return Err(io::ErrorKind::NotFound.into()),
            _ => return Err(e),
        }
    }
    // Pin each parent directory and never follow a symlink on the fallback path. A
    // realpath check followed by open would let a replacement invalidate the check.
    let mut dir = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
        .open(root)?;
    let mut parts = rel
        .split(|b| *b == b'/')
        .filter(|p| !p.is_empty())
        .peekable();
    while let Some(part) = parts.next() {
        if part == b".." {
            return Err(io::ErrorKind::NotFound.into());
        }
        let name = CString::new(part).map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
        let last = parts.peek().is_none();
        let flags = if last {
            flags
        } else {
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC
        };
        // SAFETY: dir owns a live fd, name is NUL terminated, and openat returns a new fd.
        let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags | libc::O_NOFOLLOW) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if last {
            return Ok(file);
        }
        dir = file;
    }
    Err(io::ErrorKind::NotFound.into())
}
// Append only to the disposable copy of src/files.rs, as a separate cfg(test) module.
// The production open_beneath function is called unchanged.
#[cfg(test)]
mod audit_closure_fallback {
    use super::*;
    use std::io::Read;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn audit_recheck_f02_real_open_beneath_race() {
        let base = std::env::temp_dir().join(format!("conduit-f02-native-{}", std::process::id()));
        let root = base.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let outside = base.join("outside");
        std::fs::write(&outside, "outside-marker").unwrap();
        let item = root.join("item");
        symlink(&outside, &item).unwrap();
        NO_OPENAT2.store(false, Relaxed);
        assert!(open_beneath(&root, b"/item").is_err(), "static outside symlink must fail");
        // Force only the fallback branch, simulating the cached ENOSYS/EPERM state.
        NO_OPENAT2.store(true, Relaxed);
        assert!(open_beneath(&root, b"/item").is_err(), "fallback must also reject static symlink");
        let stop = Arc::new(AtomicBool::new(false));
        let actor_stop = stop.clone();
        let actor_root = root.clone();
        let actor_outside = outside.clone();
        let actor = std::thread::spawn(move || {
            let swap = actor_root.join("swap");
            while !actor_stop.load(Relaxed) {
                std::fs::write(&swap, "inside-marker").unwrap();
                std::fs::rename(&swap, actor_root.join("item")).unwrap();
                symlink(&actor_outside, &swap).unwrap();
                std::fs::rename(&swap, actor_root.join("item")).unwrap();
            }
        });
        let mut observed = None;
        for attempt in 0..100_000 {
            if let Ok(mut file) = open_beneath(&root, b"/item") {
                let mut text = String::new();
                if file.read_to_string(&mut text).is_ok() && text == "outside-marker" {
                    observed = Some(attempt);
                    break;
                }
            }
        }
        stop.store(true, Relaxed);
        actor.join().unwrap();
        NO_OPENAT2.store(false, Relaxed);
        std::fs::write(root.join("plain"), "inside").unwrap();
        assert!(open_beneath(&root, b"/plain").is_ok());
        std::fs::remove_dir_all(&base).unwrap();
        assert!(observed.is_none(), "outside content escaped containment");
        println!("F02: closure: outside marker attempts observed {:?}; static symlink controls rejected", observed);
    }
}
