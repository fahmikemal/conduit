//! `conduit import-npm`: turn NPM's generated proxy host files into a conduit config.
//!
//! A migration aid. The generated nginx files hold everything conduit needs, and reading
//! them avoids NPM's database, which also stores credentials. Anything that is not part of
//! NPM's template (the "Advanced" tab) is copied as a TODO comment to translate into
//! plugins by hand.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct NpmHost {
    pub id: u64,
    pub hosts: Vec<String>,
    pub scheme: String,
    pub server: String,
    pub port: String,
    pub cert: Option<String>,
    pub force_https: bool,
    pub block_exploits: bool,
    pub hsts: Option<String>,
    pub cache_assets: bool,
    pub access_list: bool,
    /// NPM's "Websockets Support": the client's `Upgrade` and `Connection` go upstream.
    pub websocket: bool,
    pub advanced: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpmCert {
    pub cert: PathBuf,
    pub key: PathBuf,
}

/// Map a path inside the NPM container to the host path under `root`.
fn host_path(root: &Path, p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("/etc/letsencrypt/") {
        root.join("letsencrypt").join(rest)
    } else if let Some(rest) = p.strip_prefix("/data/") {
        root.join("data").join(rest)
    } else {
        PathBuf::from(p)
    }
}

/// The `n`th whitespace-separated word, without `;` and quotes.
fn word(line: &str, n: usize) -> &str {
    line.split_whitespace()
        .nth(n)
        .unwrap_or("")
        .trim_end_matches(';')
        .trim_matches('"')
}

/// The argument of a directive: `ssl_certificate <arg>;`.
fn arg(line: &str) -> &str {
    word(line, 1)
}

/// The value of a `set $var <value>;` line.
fn set_value(line: &str) -> &str {
    word(line, 2)
}

/// NPM's own comment lines; anything else is the operator's and is kept.
fn npm_comment(t: &str) -> bool {
    let body = t.trim_start_matches('#').trim();
    body.is_empty()
        || body.starts_with("---")
        || matches!(
            body,
            "Let's Encrypt SSL"
                | "Custom SSL"
                | "Block Exploits"
                | "Force SSL"
                | "Proxy!"
                | "Custom"
                | "HSTS (ngx_http_headers_module is required) (63072000 seconds = 2 years)"
                | "Asset Caching"
        )
        || body.split([' ', ',']).filter(|w| !w.is_empty()).all(|w| {
            w.contains('.')
                && w.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'*')
        })
}

#[derive(PartialEq)]
enum Block {
    Template,
    Advanced,
}

pub fn parse_host(
    id: u64,
    text: &str,
    root: &Path,
    certs: &mut BTreeMap<String, NpmCert>,
) -> NpmHost {
    let mut h = NpmHost {
        id,
        ..Default::default()
    };
    let mut stack: Vec<Block> = Vec::new();
    let mut map_hsts: Option<String> = None;
    let mut sts_header = false;
    let mut cert_path: Option<String> = None;
    let mut key_path: Option<String> = None;

    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if stack.last() == Some(&Block::Advanced) {
            h.advanced.push(t.to_string());
            if t.ends_with('{') {
                stack.push(Block::Advanced);
            } else if t == "}" {
                stack.pop();
            }
            continue;
        }
        if t == "}" {
            stack.pop();
            continue;
        }
        if t.ends_with('{') {
            let template =
                t == "server {" || t == "location / {" || t.starts_with("map $scheme $hsts_header");
            if template {
                stack.push(Block::Template);
            } else {
                h.advanced.push(t.to_string());
                stack.push(Block::Advanced);
            }
            continue;
        }
        if t.starts_with('#') {
            if !npm_comment(t) {
                h.advanced.push(t.to_string());
            }
            continue;
        }
        if t.starts_with("https ") && t.contains("max-age") {
            map_hsts = t.split('"').nth(1).map(str::to_string);
        } else if t.starts_with("set $forward_scheme ") {
            h.scheme = set_value(t).to_string();
        } else if t.starts_with("set $server ") {
            h.server = set_value(t).to_string();
        } else if t.starts_with("set $port ") {
            h.port = set_value(t).to_string();
        } else if let Some(names) = t.strip_prefix("server_name ") {
            h.hosts = names
                .trim_end_matches(';')
                .split_whitespace()
                .map(str::to_string)
                .collect();
        } else if t.starts_with("ssl_certificate_key ") {
            key_path = Some(arg(t).to_string());
        } else if t.starts_with("ssl_certificate ") {
            cert_path = Some(arg(t).to_string());
        } else if t.contains("conf.d/include/block-exploits.conf") {
            h.block_exploits = true;
        } else if t.contains("conf.d/include/force-ssl.conf") {
            h.force_https = true;
        } else if t.contains("conf.d/include/assets.conf") {
            h.cache_assets = true;
        } else if t.starts_with("include /data/access/") {
            h.access_list = true;
        } else if t.starts_with("add_header Strict-Transport-Security") {
            sts_header = true;
        } else if t == "proxy_set_header Upgrade $http_upgrade;" {
            h.websocket = true;
        } else if t.starts_with("listen ")
            || t.starts_with("http2 ")
            || t.starts_with("include conf.d/include/")
            || t.starts_with("include /data/nginx/custom/")
            || t.starts_with("access_log ")
            || t.starts_with("error_log ")
            || t.starts_with("set $trust_forwarded_proto ")
            || t == "proxy_set_header Connection $http_connection;"
            || t == "proxy_http_version 1.1;"
        {
            // part of NPM's template
        } else {
            h.advanced.push(t.to_string());
        }
    }
    if sts_header {
        h.hsts = map_hsts.or_else(|| Some("max-age=63072000; preload".into()));
    }
    if let (Some(c), Some(k)) = (cert_path, key_path) {
        let cert = host_path(root, &c);
        let name = cert.parent().and_then(Path::file_name).map_or_else(
            || format!("npm-host-{id}"),
            |n| n.to_string_lossy().into_owned(),
        );
        certs.entry(name.clone()).or_insert(NpmCert {
            cert,
            key: host_path(root, &k),
        });
        h.cert = Some(name);
    }
    h
}

/// YAML double-quoted string.
fn q(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if yaml_printable(c) => out.push(c),
            c => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
        }
    }
    out.push('"');
    out
}

/// What YAML allows in a document as is (YAML 1.2, `c-printable` minus line breaks); NPM
/// files may hold anything, and one NUL or stray CR would break or rewrite the config.
fn yaml_printable(c: char) -> bool {
    matches!(c, '\t' | ' '..='~' | '\u{a0}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}')
        || c >= '\u{10000}'
}

/// Text for a `#` comment: one line, nothing a YAML reader refuses.
fn comment(s: &str) -> Cow<'_, str> {
    if s.chars().all(yaml_printable) {
        return Cow::Borrowed(s);
    }
    Cow::Owned(
        s.chars()
            .map(|c| {
                if yaml_printable(c) {
                    c.to_string()
                } else {
                    format!("\\u{{{:x}}}", u32::from(c))
                }
            })
            .collect(),
    )
}

pub fn render(
    root: &Path,
    hosts: &[NpmHost],
    certs: &BTreeMap<String, NpmCert>,
    default_page: bool,
) -> String {
    let mut y = String::new();
    let _ = writeln!(
        y,
        "# Generated by `conduit import-npm` from {}.",
        root.display()
    );
    y.push_str("# Review the TODO comments before switching traffic.\n\n");
    y.push_str(
        "# Process messages go to stderr, which systemd sends to the journal.\nserver:\n  threads: 2\n\n",
    );
    y.push_str("listen:\n  http: [\"0.0.0.0:80\", \"[::]:80\"]\n  https: [\"0.0.0.0:443\", \"[::]:443\"]\n\n");
    y.push_str(
        "# NPM used 90s for every timeout; a LAN upstream that has not accepted in 10s is down.\ndefaults:\n  connect_timeout: 10s\n  read_timeout: 90s\n  write_timeout: 90s\n  max_body_size: 2000m\n\n",
    );
    y.push_str(
        "# One JSON object per request. Log readers such as CrowdSec need a JSON parser\n# instead of the nginx-proxy-manager one.\naccess_log:\n  path: /var/log/conduit/access.log\n\n",
    );
    y.push_str("# /metrics, /healthz and /status; keep it on loopback.\nadmin:\n  listen: \"127.0.0.1:9180\"\n\n");
    y.push_str(
        "# NPM trusts X-Real-IP from CDN ranges, but Cloudflare sends CF-Connecting-IP, so NPM\n# logs Cloudflare edge addresses as clients. This reads the header Cloudflare sends.\nreal_ip:\n  header: CF-Connecting-IP\n  trusted: [cloudflare]\n\n",
    );
    y.push_str("tls:\n");
    let _ = writeln!(
        y,
        "  acme_webroot: {}",
        q(&root
            .join("data/letsencrypt-acme-challenge")
            .to_string_lossy())
    );
    // An empty `certs:` reads as null, which the config refuses.
    y.push_str(if certs.is_empty() {
        "  certs: []\n"
    } else {
        "  certs:\n"
    });
    for (name, c) in certs {
        let _ = writeln!(y, "    - name: {}", q(name));
        let _ = writeln!(y, "      cert: {}", q(&c.cert.to_string_lossy()));
        let _ = writeln!(y, "      key: {}", q(&c.key.to_string_lossy()));
    }
    if default_page {
        let _ = writeln!(
            y,
            "\n# NPM \"Default Site\": custom page for hosts that match no site.\ndefault_site:\n  page: {}",
            q(&root
                .join("data/nginx/default_www/index.html")
                .to_string_lossy())
        );
    }
    y.push_str("\nsites:\n");
    for h in hosts {
        let _ = writeln!(y, "  - id: {}", q(&h.id.to_string()));
        let names: Vec<String> = h.hosts.iter().map(|n| q(n)).collect();
        let _ = writeln!(y, "    hosts: [{}]", names.join(", "));
        let _ = writeln!(
            y,
            "    upstream: {}",
            q(&format!("{}://{}:{}", h.scheme, h.server, h.port))
        );
        if h.server == "127.0.0.1" && h.port == "81" {
            y.push_str("    enabled: false  # NPM's own admin UI, which goes away with NPM\n");
        }
        if let Some(c) = &h.cert {
            let _ = writeln!(y, "    cert: {}", q(c));
        }
        if h.force_https {
            y.push_str("    force_https: true\n");
        }
        if let Some(v) = &h.hsts {
            let _ = writeln!(y, "    hsts: {}", q(v));
        }
        // NPM passes any upgrade when "Websockets Support" is on (headscale needs that), and
        // none when it is off.
        y.push_str(if h.websocket {
            "    upgrade: any\n"
        } else {
            "    upgrade: off\n"
        });
        if h.cache_assets {
            y.push_str(
                "    # NPM \"Cache Assets\" was on. conduit caches what the upstream allows\n    \
                 # (Cache-Control); to also keep assets that state nothing for 30 minutes:\n    \
                 # cache: { match: { path: { regex: \"(?i)\\\\.(css|js|jpe?g|gif|png|webp|woff2?|ttf|svg|ico)$\" } }, default_ttl: 30m }\n",
            );
        }
        if h.access_list {
            y.push_str(
                "    # TODO: NPM access list was attached; use the ip_filter and basic_auth plugins.\n",
            );
        }
        if !h.advanced.is_empty() {
            y.push_str(
                "    # TODO: NPM advanced config; translate into plugins and site options:\n",
            );
            for line in &h.advanced {
                let _ = writeln!(y, "    #   {}", comment(line));
            }
        }
        // NPM includes block-exploits.conf before anything else of the host, so the plugin
        // comes first; translated advanced rules go after it.
        if h.block_exploits {
            y.push_str(
                "    plugins:\n      - block_exploits: {}  # NPM \"Block Exploits\" was on\n",
            );
        }
    }
    y
}

/// Reads the NPM directory at `root`; paths in the output point into `paths_root`.
pub fn run(root: &Path, paths_root: &Path, out: &mut impl Write) -> Result<(), String> {
    let dir = root.join("data/nginx/proxy_host");
    let mut files: Vec<(u64, PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|p| {
            let id = p
                .file_name()?
                .to_str()?
                .strip_suffix(".conf")?
                .parse()
                .ok()?;
            Some((id, p))
        })
        .collect();
    files.sort();
    let mut certs = BTreeMap::new();
    let mut hosts = Vec::new();
    for (id, path) in files {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let h = parse_host(id, &text, paths_root, &mut certs);
        if h.hosts.is_empty() || h.server.is_empty() {
            eprintln!("skipping {}: no server_name or upstream", path.display());
            continue;
        }
        hosts.push(h);
    }
    let todo = hosts
        .iter()
        .filter(|h| !h.advanced.is_empty() || h.access_list)
        .count();
    // NPM's "Default Site" set to a custom page: it is served for every path on unknown hosts.
    let default_page = std::fs::read_to_string(root.join("data/nginx/default_host/site.conf"))
        .is_ok_and(|t| t.contains("root /data/nginx/default_www;"))
        && root.join("data/nginx/default_www/index.html").is_file();
    out.write_all(render(paths_root, &hosts, &certs, default_page).as_bytes())
        .map_err(|e| e.to_string())?;
    eprintln!(
        "imported {} proxy hosts and {} certificates; {todo} need manual review (TODO)",
        hosts.len(),
        certs.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# ------------------------------------------------------------
# git.example.com
# ------------------------------------------------------------
map $scheme $hsts_header {
    https   "max-age=63072000; preload";
}
server {
  set $forward_scheme http;
  set $server         "10.0.0.4";
  set $port           3000;
  listen 80;
listen [::]:80;
listen 443 ssl;
listen [::]:443 ssl;
  server_name git.example.com	;
  http2 on;
  # Let's Encrypt SSL
  include conf.d/include/letsencrypt-acme-challenge.conf;
  include conf.d/include/ssl-cache.conf;
  include conf.d/include/ssl-ciphers.conf;
  ssl_certificate /etc/letsencrypt/live/npm-57/fullchain.pem;
  ssl_certificate_key /etc/letsencrypt/live/npm-57/privkey.pem;
  # Block Exploits
  include conf.d/include/block-exploits.conf;
    # Force SSL
    set $trust_forwarded_proto "F";
    include conf.d/include/force-ssl.conf;
proxy_set_header Upgrade $http_upgrade;
proxy_set_header Connection $http_connection;
proxy_http_version 1.1;
  access_log /data/logs/proxy-host-8_access.log proxy;
  error_log /data/logs/proxy-host-8_error.log warn;
# Moved 6 Nov: browsers go to the new name.
if ($request_method = OPTIONS) {
    return 204;
}
location = /secret.log {
      deny all;
      return 404;
}
  location / {
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $http_connection;
    proxy_http_version 1.1;
    # Proxy!
    include conf.d/include/proxy.conf;
  }
  # Custom
  include /data/nginx/custom/server_proxy[.]conf;
}
"#;

    #[test]
    fn parses_template_and_keeps_advanced() {
        let mut certs = BTreeMap::new();
        let h = parse_host(8, SAMPLE, Path::new("/opt/npm"), &mut certs);
        assert_eq!(h.hosts, vec!["git.example.com"]);
        assert_eq!(
            (h.scheme.as_str(), h.server.as_str(), h.port.as_str()),
            ("http", "10.0.0.4", "3000")
        );
        assert!(h.force_https && h.block_exploits && h.websocket);
        assert_eq!(h.cert.as_deref(), Some("npm-57"));
        assert_eq!(
            certs["npm-57"],
            NpmCert {
                cert: "/opt/npm/letsencrypt/live/npm-57/fullchain.pem".into(),
                key: "/opt/npm/letsencrypt/live/npm-57/privkey.pem".into(),
            }
        );
        assert_eq!(
            h.advanced,
            vec![
                "# Moved 6 Nov: browsers go to the new name.",
                "if ($request_method = OPTIONS) {",
                "return 204;",
                "}",
                "location = /secret.log {",
                "deny all;",
                "return 404;",
                "}",
            ]
        );
        assert!(h.hsts.is_none());
    }

    #[test]
    fn rendered_yaml_is_a_valid_config() {
        let mut certs = BTreeMap::new();
        let h = parse_host(8, SAMPLE, Path::new("/opt/npm"), &mut certs);
        let yaml = render(Path::new("/opt/npm"), &[h], &certs, true);
        let cfg = crate::config::parse(&yaml).expect(&yaml);
        assert_eq!(cfg.sites[0].id, "8");
        assert!(matches!(
            &cfg.sites[0].upstream,
            Some(crate::config::UpstreamRef::Inline(p)) if p.backends[0].url == "http://10.0.0.4:3000"
        ));
        assert_eq!(cfg.sites[0].cert.as_deref(), Some("npm-57"));
        assert_eq!(cfg.sites[0].upgrade, crate::config::UpgradeMode::Any);
        assert!(matches!(
            &cfg.sites[0].plugins[..],
            [crate::config::PluginConfig::BlockExploits(b)] if *b == Default::default()
        ));
        assert_eq!(cfg.tls.certs.len(), 1);
        assert_eq!(
            cfg.access_log.path.as_deref(),
            Some("/var/log/conduit/access.log")
        );
        assert_eq!(cfg.admin.listen.as_deref(), Some("127.0.0.1:9180"));
        assert_eq!(
            cfg.default_site.page.as_deref(),
            Some("/opt/npm/data/nginx/default_www/index.html")
        );
        assert!(yaml.contains("#   location = /secret.log {"));
    }

    #[test]
    fn websocket_support_maps_to_upgrade() {
        let mut certs = BTreeMap::new();
        let without: String = SAMPLE
            .lines()
            .filter(|l| !l.contains("$http_upgrade") && !l.contains("$http_connection"))
            .map(|l| format!("{l}\n"))
            .collect();
        let off = parse_host(9, &without, Path::new("/opt/npm"), &mut certs);
        let on = parse_host(8, SAMPLE, Path::new("/opt/npm"), &mut certs);
        assert!(on.websocket && !off.websocket);
        let cfg = crate::config::parse(&render(Path::new("/opt/npm"), &[on, off], &certs, false))
            .unwrap();
        use crate::config::UpgradeMode::{Any, Off};
        assert_eq!((cfg.sites[0].upgrade, cfg.sites[1].upgrade), (Any, Off));
    }

    /// Found by fuzzing: control characters in an NPM file (a NUL in the advanced config, a
    /// stray CR) were copied as is, and the config written did not load. A CR could also end
    /// a comment line and turn the rest into live YAML.
    #[test]
    fn control_characters_stay_inert() {
        let text = SAMPLE
            .replace("deny all;", "deny all;\0\x01\rsites: []")
            .replace(
                "server_name git.example.com",
                "server_name git.example.com\x7f",
            );
        let mut certs = BTreeMap::new();
        let h = parse_host(8, &text, Path::new("/opt/npm"), &mut certs);
        let yaml = render(Path::new("/opt/npm"), &[h], &certs, false);
        let cfg = crate::config::parse(&yaml).expect(&yaml);
        assert_eq!(cfg.sites.len(), 1);
        assert!(
            yaml.contains("#   deny all;\\u{0}\\u{1}\\u{d}sites: []"),
            "{yaml}"
        );
        assert!(
            cfg.sites[0]
                .hosts
                .iter()
                .any(|n| n == "git.example.com\u{7f}")
        );
    }

    /// Found by fuzzing: an NPM install without certificates got an empty `certs:`, which
    /// does not load.
    #[test]
    fn no_certificates_still_loads() {
        let text: String = SAMPLE
            .lines()
            .filter(|l| !l.contains("ssl_certificate"))
            .map(|l| format!("{l}\n"))
            .collect();
        let mut certs = BTreeMap::new();
        let h = parse_host(8, &text, Path::new("/opt/npm"), &mut certs);
        assert!(certs.is_empty());
        let yaml = render(Path::new("/opt/npm"), &[h], &certs, false);
        let cfg = crate::config::parse(&yaml).expect(&yaml);
        assert!(cfg.tls.certs.is_empty());
    }
}
