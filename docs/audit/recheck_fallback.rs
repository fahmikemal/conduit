// Append only to the disposable copy of src/files.rs, as a separate cfg(test) module.
// The production open_beneath function is called unchanged.
#[cfg(test)]
mod audit_recheck_fallback {
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
        std::fs::remove_dir_all(&base).unwrap();
        assert!(observed.is_some(), "race not observed in this run; not proof of safety");
        println!("F02: unchanged production open_beneath read outside marker at attempt {:?}; static symlink controls rejected", observed.unwrap());
    }
}
