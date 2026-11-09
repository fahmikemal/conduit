//! Background service: starts the access log writer, then rebuilds the runtime on SIGHUP or
//! when the config, a certificate or a served file changes on disk (e.g. a certbot renewal).
//! A bad config is reported and the running one is kept. Upstream pools whose definition did
//! not change are carried over, so health and ejection state survive the reload.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use log::{error, info, warn};
use pingora::server::ShutdownWatch;
use pingora::services::background::BackgroundService;
use tokio::signal::unix::{SignalKind, signal};

use crate::config;
use crate::metrics;
use crate::proxy::{ReloadStatus, Shared};
use crate::runtime;

type Stamp = Option<(SystemTime, u64, u64)>;

fn stamp(path: &PathBuf) -> Stamp {
    // metadata() follows symlinks, so a certbot renewal (new symlink target) shows up.
    std::fs::metadata(path).ok().map(|m| {
        (
            m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            m.ino(),
            m.len(),
        )
    })
}

fn snapshot(paths: &[PathBuf]) -> Vec<(PathBuf, Stamp)> {
    paths.iter().map(|p| (p.clone(), stamp(p))).collect()
}

pub struct Reloader {
    shared: Arc<Shared>,
    interval: Duration,
    stamps: Mutex<Vec<(PathBuf, Stamp)>>,
}

impl Reloader {
    pub fn new(shared: Arc<Shared>, interval: Duration) -> Reloader {
        let stamps = snapshot(&shared.runtime.load().watched);
        Reloader {
            shared,
            interval,
            stamps: Mutex::new(stamps),
        }
    }

    fn changed(&self) -> bool {
        self.stamps
            .lock()
            .unwrap()
            .iter()
            .any(|(path, old)| stamp(path) != *old)
    }

    fn reload(&self, why: &str) {
        let path = &self.shared.config_path;
        // Stamps from before anything is read. A file replaced while the load reads it (a
        // renewal swapping the key right after the certificate) then still differs from
        // what is recorded, and the next tick loads it again. Stamping after the load
        // would record the new file next to the old content and never look at it again.
        let before: HashMap<PathBuf, Stamp> = {
            let mut paths: Vec<PathBuf> = self
                .stamps
                .lock()
                .unwrap()
                .iter()
                .map(|(p, _)| p.clone())
                .collect();
            if !paths.contains(path) {
                paths.push(path.clone());
            }
            snapshot(&paths).into_iter().collect()
        };
        // Files the new configuration adds are stamped now; they were not watched before.
        let recorded = |paths: &[PathBuf]| -> Vec<(PathBuf, Stamp)> {
            paths
                .iter()
                .map(|p| {
                    (
                        p.clone(),
                        before.get(p).copied().unwrap_or_else(|| stamp(p)),
                    )
                })
                .collect()
        };
        let current = self.shared.runtime.load_full();
        let result = config::load(path).and_then(|cfg| {
            let s = &self.shared;
            if cfg.listen != s.listen
                || cfg.admin != s.admin
                || cfg.access_log.path.as_deref().map(Path::new) != s.access_log.path()
            {
                warn!(
                    "listen, admin or access_log.path changed; that part applies after a \
                     restart or graceful upgrade"
                );
            }
            runtime::build(&cfg, Some(path.clone()), Some(&current))
        });
        match result {
            Ok(built) => {
                for w in &built.warnings {
                    warn!("{w}");
                }
                for gone in current
                    .sites
                    .iter()
                    .filter(|old| !built.runtime.sites.iter().any(|s| s.id == old.id))
                {
                    metrics::forget_site(&gone.id);
                }
                if built.runtime.cache.lock_timeout != current.cache.lock_timeout {
                    warn!(
                        "cache.lock_timeout changed; that applies after a restart or graceful upgrade"
                    );
                }
                crate::cache::apply(&built.runtime.cache);
                let caching: Vec<&str> = built
                    .runtime
                    .sites
                    .iter()
                    .filter(|s| s.cache.is_some())
                    .map(|s| &*s.id)
                    .collect();
                let dropped = crate::cache::retain_sites(&caching);
                if dropped > 0 {
                    info!("dropped {dropped} cached responses of sites that no longer cache");
                }
                let hosts = built.runtime.router.len();
                let pools = built.runtime.pools.len();
                let kept = built
                    .runtime
                    .pools
                    .iter()
                    .filter(|(name, p)| current.pools.get(*name).is_some_and(|o| Arc::ptr_eq(o, p)))
                    .count();
                *self.stamps.lock().unwrap() = recorded(&built.runtime.watched);
                self.shared.certs.store(Arc::new(built.certs));
                pingora::protocols::http::set_server_header(built.runtime.server_header.clone());
                self.shared.runtime.store(Arc::new(built.runtime));
                record(&self.shared, None);
                self.shared.acme_wake.notify_one();
                info!(
                    "reloaded after {why}: {hosts} host names, {pools} upstream pools ({kept} kept their state)"
                );
            }
            Err(e) => {
                // Remember the files as they were read so the same broken state is not
                // retried every tick, while a fix written meanwhile still triggers a reload.
                let paths: Vec<PathBuf> = before.keys().cloned().collect();
                *self.stamps.lock().unwrap() = recorded(&paths);
                error!("reload after {why} failed, keeping the running config: {e}");
                record(&self.shared, Some(e));
            }
        }
    }
}

/// Note the outcome of a configuration load for `/status` and the metrics.
pub fn record(shared: &Shared, error: Option<String>) {
    let now = SystemTime::now();
    let ok = error.is_none();
    metrics::RELOADS
        .with_label_values(&[if ok { "success" } else { "failure" }])
        .inc();
    if ok {
        metrics::LAST_RELOAD.set(
            now.duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
        );
    }
    *shared.last_reload.lock().unwrap() = ReloadStatus { at: now, ok, error };
}

#[async_trait]
impl BackgroundService for Reloader {
    async fn start(&self, mut shutdown: ShutdownWatch) {
        self.shared.access_log.start();
        let mut hup = match signal(SignalKind::hangup()) {
            Ok(s) => Some(s),
            Err(e) => {
                error!("cannot listen for SIGHUP: {e}");
                None
            }
        };
        let mut tick = tokio::time::interval(self.interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tick.tick().await;
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    self.shared.access_log.flush(Duration::from_secs(2));
                    return;
                }
                Some(()) = async {
                    match hup.as_mut() {
                        Some(h) => h.recv().await,
                        None => std::future::pending().await,
                    }
                } => self.reload("SIGHUP"),
                _ = self.shared.reload_now.notified() => self.reload("a new ACME certificate"),
                _ = tick.tick() => {
                    if self.changed() {
                        self.reload("a file change");
                    }
                }
            }
        }
    }
}
