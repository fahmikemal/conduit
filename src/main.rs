//! conduit: a reverse proxy built on Pingora.

use conduit::acme;
use conduit::admin;
use conduit::cache;
use conduit::config;
use conduit::crowdsec;
use conduit::import;
use conduit::logger;
use conduit::metrics;
use conduit::reload;
use conduit::runtime;
use conduit::tls;
use conduit::upstream;

use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use arc_swap::ArcSwap;
use clap::{Parser, Subcommand};
use log::{info, warn};
use pingora::listeners::tls::TlsSettings;
use pingora::listeners::{L4BufferSettings, ListenerConfig, TcpSocketOptions};
use pingora::proxy::http_proxy_service_with_name;
use pingora::server::configuration::{Opt, ServerConf};
use pingora::server::{RunArgs, Server};
use pingora::services::background::background_service;

use conduit::accesslog::AccessLog;
use conduit::proxy::{Conduit, ReloadStatus, Shared};

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// `[::]` is IPv6-only so it can sit next to `0.0.0.0` on the same port instead of failing
/// with "address in use" on dual-stack hosts.
fn socket_options(addr: &str) -> TcpSocketOptions {
    let mut o = TcpSocketOptions::default();
    if addr.starts_with('[') {
        o.ipv6_only = Some(true);
    }
    o
}

/// A listening endpoint. Pingora serializes the response header into the connection's
/// write buffer and flushes it before any body that does not fit; with its 1460-byte
/// default a 2 KB response goes out as two packets, and the client ACKs the first one on
/// its own. A larger buffer sends header and body together.
fn listener(addr: &str, write_buffer: usize) -> ListenerConfig {
    ListenerConfig::tcp(addr)
        .tcp_socket_options(socket_options(addr))
        .l4_buffer(L4BufferSettings::default().write(write_buffer))
}

const DEFAULT_CONFIG: &str = "/etc/conduit/conduit.yaml";

#[derive(Parser)]
#[command(name = "conduit", version, about = "Reverse proxy built on Pingora")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the proxy
    Run {
        #[arg(short, long, default_value = DEFAULT_CONFIG)]
        config: PathBuf,
        /// Run in the background (needed for graceful upgrades under systemd)
        #[arg(short, long)]
        daemon: bool,
        /// Take the listening sockets over from a running conduit (graceful upgrade)
        #[arg(short, long)]
        upgrade: bool,
    },
    /// Validate the configuration and load every certificate, then exit
    Check {
        #[arg(short, long, default_value = DEFAULT_CONFIG)]
        config: PathBuf,
    },
    /// Print a conduit config converted from Nginx Proxy Manager's proxy hosts (migration aid)
    ImportNpm {
        /// NPM directory on the host, holding data/ and letsencrypt/
        #[arg(long, default_value = "/opt/npm")]
        npm_root: PathBuf,
        /// Where that directory lives on the target host, when importing a copy of it
        #[arg(long)]
        paths_root: Option<PathBuf>,
    },
}

/// Opt this process in or out of transparent huge pages (`server.transparent_huge_pages`).
/// The setting survives fork and exec, so it covers the daemonized process too. Pages
/// already faulted in keep their size, hence the opt-out first thing in `main`.
fn huge_pages(enabled: bool) {
    // SAFETY: PR_SET_THP_DISABLE takes integer arguments only.
    let r = unsafe {
        libc::prctl(
            libc::PR_SET_THP_DISABLE,
            libc::c_ulong::from(!enabled),
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    };
    if r != 0 {
        warn!(
            "transparent huge pages setting: {}",
            std::io::Error::last_os_error()
        );
    }
}

fn main() -> ExitCode {
    huge_pages(false);
    let cli = Cli::parse();
    logger::init();
    let result = match cli.cmd {
        Cmd::Run {
            config,
            daemon,
            upgrade,
        } => run(&config, daemon, upgrade),
        Cmd::Check { config } => check(&config),
        Cmd::ImportNpm {
            npm_root,
            paths_root,
        } => import::run(
            &npm_root,
            paths_root.as_deref().unwrap_or(&npm_root),
            &mut std::io::stdout().lock(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("conduit: {e}");
            ExitCode::FAILURE
        }
    }
}

fn check(path: &Path) -> Result<(), String> {
    let cfg = config::load(path)?;
    let built = runtime::build(&cfg, Some(path.to_path_buf()), None)?;
    println!(
        "config ok: {} sites, {} host names, {} upstream pools",
        built.runtime.sites.len(),
        built.runtime.router.len(),
        built.runtime.pools.len()
    );
    for p in built.runtime.pools.values() {
        let backends: Vec<String> = p.status().map(|b| b.label.to_string()).collect();
        println!(
            "  upstream {} ({:?}): {}",
            p.name,
            p.balance,
            backends.join(", ")
        );
    }
    for c in built.certs.summary() {
        println!("  cert {c}");
    }
    if let Some(plan) = &built.runtime.crowdsec {
        let appsec = if plan.appsec.is_some() {
            ", appsec"
        } else {
            ""
        };
        println!("  crowdsec {}{appsec}", plan.lapi_url);
    }
    if let Some(plan) = &built.runtime.acme {
        println!("  acme {} (storage {})", plan.directory, plan.dir.display());
        for site in &plan.sites {
            let stored = if site.path.exists() {
                "stored"
            } else {
                "not issued yet"
            };
            println!("    site {}: {} ({stored})", site.id, site.hosts.join(", "));
        }
    }
    for w in &built.warnings {
        println!("  warning: {w}");
    }
    Ok(())
}

fn run(path: &Path, daemon: bool, upgrade: bool) -> Result<(), String> {
    let cfg = config::load(path)?;
    if cfg.server.transparent_huge_pages {
        huge_pages(true);
    }
    metrics::init();
    cache::apply(&cfg.cache);
    let built = runtime::build(&cfg, Some(path.to_path_buf()), None)?;
    for w in &built.warnings {
        warn!("{w}");
    }
    // Pingora's own responses (an unparsable request, CONNECT) say what conduit's say.
    pingora::protocols::http::set_server_header(built.runtime.server_header.clone());
    info!(
        "conduit {}: {} host names, {} upstream pools, {} certificates",
        env!("CARGO_PKG_VERSION"),
        built.runtime.router.len(),
        built.runtime.pools.len(),
        cfg.tls.certs.len()
    );

    let shared = Arc::new(Shared {
        runtime: ArcSwap::from_pointee(built.runtime),
        certs: Arc::new(ArcSwap::from_pointee(built.certs)),
        access_log: AccessLog::new(
            cfg.access_log.path.as_ref().map(PathBuf::from),
            cfg.access_log.queue_capacity,
            cfg.server.threads,
        ),
        config_path: path.to_path_buf(),
        listen: cfg.listen.clone(),
        admin: cfg.admin.clone(),
        started: SystemTime::now(),
        last_reload: Mutex::new(ReloadStatus {
            at: SystemTime::now(),
            ok: true,
            error: None,
        }),
        reload_now: tokio::sync::Notify::new(),
        acme_wake: tokio::sync::Notify::new(),
        acme: acme::Status::default(),
        // With work stealing a request may move between threads; there is no worker to keep
        // connections on.
        pool_per_worker: !cfg.server.work_stealing && cfg.server.threads > 1,
    });
    reload::record(&shared, None);

    let s = &cfg.server;
    let conf = ServerConf {
        version: 1,
        threads: s.threads.max(1),
        work_stealing: s.work_stealing,
        pid_file: s.pid_file.clone(),
        upgrade_sock: s.upgrade_sock.clone(),
        error_log: s.error_log.clone(),
        daemon,
        // The parent of a daemon exits only once the child is ready to take over the
        // listeners, so systemd sends SIGQUIT to the old process at the right time and a
        // reload whose new process fails aborts instead of leaving nothing listening.
        daemon_wait_for_ready: daemon,
        daemon_ready_timeout_seconds: NonZeroU64::new(60),
        upstream_keepalive_pool_size: s.upstream_keepalive_pool_size,
        grace_period_seconds: Some(s.shutdown_grace.0.as_secs()),
        graceful_shutdown_timeout_seconds: Some(s.shutdown_timeout.0.as_secs()),
        ..ServerConf::default()
    };
    let opt = Opt {
        upgrade,
        daemon,
        nocapture: false,
        test: false,
        conf: None,
    };
    let mut server = Server::new_with_opt_and_conf(opt, conf);
    // Bootstrap (taking over listeners from the old process on upgrade) runs after the fork.
    let bootstrap = server.bootstrap_as_a_service();

    let mut proxy = http_proxy_service_with_name(
        &server.configuration,
        Conduit::new(shared.clone()),
        "conduit",
    );
    let write_buffer = cfg.server.write_buffer.0.unwrap_or(0) as usize;
    for addr in &cfg.listen.http {
        proxy.add_listener(listener(addr, write_buffer));
    }
    for addr in &cfg.listen.https {
        let resolver = tls::CertResolver::new(shared.certs.clone());
        let mut settings = TlsSettings::with_callbacks(Box::new(resolver))
            .map_err(|e| format!("tls settings: {e}"))?;
        settings.enable_h2();
        tls::ask_client_certs(&mut settings, shared.clone());
        proxy.add_listener(listener(addr, write_buffer).tls(settings));
    }
    server.add_service(proxy).add_dependency(&bootstrap);
    if let Some(addr) = &cfg.admin.listen {
        server
            .add_service(admin::service(shared.clone(), addr))
            .add_dependency(&bootstrap);
    }
    server.add_service(background_service(
        "health",
        upstream::HealthService::new(shared.clone()),
    ));
    server.add_service(background_service(
        "acme",
        acme::AcmeService::new(shared.clone()),
    ));
    server.add_service(background_service(
        "crowdsec",
        crowdsec::Service::new(shared.clone()),
    ));
    server.add_service(background_service(
        "reload",
        reload::Reloader::new(shared.clone(), cfg.tls.watch_interval.0),
    ));
    server.run(RunArgs::default());
    // Every runtime has stopped. Connections still open then (upgraded ones) were dropped and
    // logged on the way out: write what is buffered before exiting.
    shared.access_log.flush(std::time::Duration::from_secs(2));
    std::process::exit(0)
}
