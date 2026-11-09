# conduit: dokumentasi teknis

Dokumentasi lengkap (bahasa Indonesia): fitur, hasil uji, benchmark, perintah, dan rencana.
Ringkasan berbahasa Inggris ada di [README](../README.md).

Reverse proxy di atas [Pingora](https://github.com/cloudflare/pingora) (0.9.1, dengan beberapa
patch performa di [`vendor/`](../vendor/README.md)). Targetnya kelas
produksi di mesin kecil (2 vCPU / 2 GB) yang sekarang menjalankan Nginx Proxy Manager.
Desainnya mengikuti model Pingora sendiri (hook `ProxyHttp`, load balancer, background
service), bukan tiruan nginx/NPM.

**Status: fase 1 dan fase 2 selesai. Fase 2: ACME, CrowdSec, rate limit, mTLS, auth, ditambah
file server statis, cache respons, dan WebSocket/upgrade. Fase 1 ditutup 2026-11-08 dengan target
throughput yang diubah dari "setara nginx" menjadi ≥0,75× nginx tuned atau CPU per request ≤1,4×:
HTTP/1.1 dan HTTPS kini 0,76–0,81× setelah patch Pingora, dari ±0,6× (lihat
[Throughput HTTP/1.1 dan HTTPS](#throughput-http11-dan-https)).
Fase 3 (stabilitas inti, tanpa fitur baru) berjalan: butir 1, 3, 4, 5, 6 selesai, soak 24 jam
berjalan. Atas permintaan, rules mendapat aksi `proxy` (upstream per path, `strip_prefix`,
`add_prefix`, `rewrite`) dan plugin baru `body_replace`, padanan `proxy_pass`/`rewrite`/`sub_filter`
nginx, serta plugin `block_exploits` (pengganti "Block Exploits" NPM). Semua diuji lokal. Belum ada yang dipasang atau diubah di server.**

## Fitur

**Routing dan TLS**
- Routing per host (exact dan `*.domain`), satu site bisa punya banyak host.
- TLS dengan pemilihan sertifikat per SNI (override site → SAN exact → wildcard → default),
  HTTP/2 ke klien, sertifikat di-reload saat file berubah.
- Force HTTPS, HSTS, batas body, timeout per site, halaman default untuk host tak dikenal,
  webroot ACME.
- Klien HTTP/1.1 yang diam di antara request diputus setelah `keepalive_timeout` (default 75
  detik, seperti nginx). `Keep-Alive: timeout=` dari klien tidak diikuti; tanpa ini Pingora
  membiarkan koneksi idle terbuka selamanya. Header request harus lengkap dalam 60 detik sejak
  byte pertama, jadi klien slowloris tidak bisa menahan koneksi (patch Pingora `7ffe1c4`).
  Header sampai 1 MiB dan 256 baris diterima, lebih dari itu ditolak 4xx. Target tanpa path
  (`GET ?x`) dijawab 400 seperti nginx, karena Pingora meneruskannya apa adanya.
- Kompresi level 1 (algoritma sesuai `Accept-Encoding`) untuk respons teks utuh berstatus
  200/403/404 yang belum dikompres upstream. Respons 206 tidak pernah dikompres, karena byte
  range-nya mengacu ke isi asli.
- Header `Server` opsional (`defaults.server_header`, mati secara bawaan). Kalau diisi, misalnya
  `server_header: conduit`, nilainya dipasang persis seperti ditulis, tanpa nomor versi, di
  setiap respons: respons proxy (menggantikan `Server` upstream), jawaban lokal (403, 404,
  redirect), halaman error 502/504, dan respons yang ditulis Pingora sendiri (400 untuk header
  rusak, 405 untuk `CONNECT`; patch Pingora `499f89d`). Kalau tidak diisi, conduit tidak
  mengirim `Server` sama sekali, termasuk nama "Pingora" yang dulu bocor di dua respons itu, dan
  `Server` upstream diteruskan apa adanya. Site bisa membuangnya lewat
  `headers: { response: { remove: [server] } }`. Berubah saat reload. Supaya terdeteksi
  Wappalyzer, fingerprint `Server: ^conduit$` tetap harus didaftarkan ke database mereka.

**WebSocket dan protocol upgrade** (`upgrade:` dan `upgrade_idle_timeout:` per site)
- `upgrade: websocket` (default): hanya handshake WebSocket yang diteruskan, dengan header
  hop-by-hop dinormalkan oleh Pingora. Upgrade lain sampai ke upstream sebagai request biasa.
- `upgrade: any`: setiap `Upgrade` HTTP/1.1 diteruskan dengan header handshake apa adanya
  (minus `Keep-Alive`, `TE`, `Proxy-Connection`, seperti nginx). Ini yang dibutuhkan headscale:
  klien Tailscale memakai `Upgrade: tailscale-control-protocol`, DERP memakai `Upgrade: DERP`.
  Sama dengan "Websockets Support" di NPM.
- `upgrade: off`: tidak ada upgrade, upstream melihat request biasa.
- Aliran setelah 101 bukan request body, jadi `max_body_size` tidak berlaku padanya. Hanya
  handshake yang benar-benar selesai yang dihitung: header `Upgrade` yang ditolak upstream tidak
  melonggarkan batas body.
- Koneksi yang sudah di-upgrade ditutup setelah `upgrade_idle_timeout` tanpa data ke arah mana
  pun (default: `read_timeout` site, 90 detik). Tanpa pengaturan ini, Pingora memakai 60 detik
  tetap untuk sisi klien.
- Request upgrade tidak pernah masuk cache. Reload (SIGHUP) tidak memutus koneksi yang sedang
  terbuka (diuji). Saat graceful upgrade binary, proses lama masih menerima koneksi
  ±5 detik (konstanta Pingora), lalu berhenti setelah `shutdown_grace` + `shutdown_timeout`.
  Koneksi upgrade yang masih terbuka ikut tertutup dan klien menyambung ulang ke proses baru.
  Masing-masing tetap tercatat sekali di access log, dengan status 101 dan `"error":"Cancelled"`
  (diuji).
- Satu baris access log per koneksi, ditulis saat koneksi ditutup, dengan status 101, durasi
  seluruh sesi, dan byte yang dikirim klien. Plugin `concurrency` memegang slot selama koneksi
  terbuka; `rate_limit` hanya menghitung handshake-nya.
- WebSocket lewat HTTP/2 (RFC 8441) tidak didukung Pingora. Browser otomatis memakai HTTP/1.1
  untuk WebSocket, begitu juga Cloudflare ke origin.

**mTLS per site** (`mtls: { ca, optional }`)
- Site meminta sertifikat klien yang ditandatangani CA di `ca` (file PEM, di-reload saat berubah).
  Tanpa sertifikat atau sertifikat tidak valid: 403 dengan alasannya (mis. "certificate has
  expired"), dicatat sebagai `"blocked_by": "mtls"`. `optional: true` meloloskan klien tanpa
  sertifikat.
- Upstream menerima `X-Client-Verify` (`SUCCESS`/`NONE`), `X-Client-Cert-Subject` (RFC 4514) dan
  `X-Client-Cert-Fingerprint` (SHA-256). Header yang sama dari klien selalu dibuang.
- Permintaan sertifikat dipasang di callback ClientHello, sebelum OpenSSL memutuskan resumption.
  Session ID context = SHA-256 file CA, jadi sesi TLS hanya bisa di-resume di bawah CA yang sama:
  sesi dari site tanpa mTLS, dari CA lain, atau dari sebelum file CA diganti tidak pernah dipakai
  ulang (klien handshake penuh dan diverifikasi lagi). Sesi mTLS yang sah tetap bisa di-resume,
  TLS 1.2 maupun 1.3.
- Koneksi yang dibuat untuk host lain (HTTP/2 connection reuse, SNI ≠ Host) dijawab 421, supaya
  browser membuka koneksi sendiri untuk host itu. Site yang berbagi file CA saling menerima.

**ACME bawaan** (`tls.acme` dan `acme: true` per site), tanpa certbot
- Satu sertifikat per site untuk semua host-nya. Terbit otomatis saat start, juga saat host
  site berubah.
- Diperpanjang di jendela waktu yang disarankan CA (ARI, RFC 9773). Kalau CA tidak memberi
  jendela, diperpanjang saat sisa masa berlaku tinggal 1/3. `renew_before` bisa mengubahnya,
  maksimal setengah masa berlaku.
- Challenge HTTP-01 dijawab conduit sendiri dari memori, di semua host, sebelum redirect HTTPS.
- Key dan chain disimpan dalam satu file PEM per site (izin 0600), diganti secara atomik.
  Sertifikat baru langsung dipasang lewat reload tanpa memutus koneksi.
- Restart memakai sertifikat yang tersimpan, tidak memesan ulang. Lock file mencegah proses
  lama dan baru memesan bersamaan saat graceful upgrade.
- Kalau order gagal: dicatat di log, di `/status`, dan di metrik
  `conduit_acme_orders_total{result="failure"}`. Percobaan berikutnya mundur bertahap: 5 menit,
  berlipat ganda, maksimal 6 jam (Let's Encrypt membatasi 5 validasi gagal per host per jam).
- Request ke CA lewat connector HTTP Pingora dan OpenSSL yang sama dengan proxy. Library
  `instant-acme` hanya dipakai untuk protokolnya, jadi tidak ada stack TLS kedua.
- Batasan: wildcard butuh DNS-01, yang belum didukung, jadi pakai file sertifikat. Port 80
  (`listen.http`) harus bisa dijangkau CA.

**CrowdSec bawaan** (bagian `crowdsec:` dan plugin `crowdsec` per site), tanpa bouncer terpisah
- Bouncer L7: keputusan (decision) ditarik dari Local API dalam mode stream, daftar penuh saat
  start lalu perubahannya tiap `update_interval` (bawaan 10 detik), dan disimpan di memori.
  Memeriksa klien cukup beberapa lookup hash (±26 ns di 100 ribu decision), tanpa panggilan
  jaringan per request.
- Yang diperiksa IP klien asli (`real_ip`), jadi di belakang Cloudflare yang diblokir
  pengunjungnya. Firewall bouncer di host tidak bisa melakukan ini, karena peer TCP-nya
  Cloudflare.
- Alamat dan range (IPv4/IPv6) disimpan apa adanya; range tidak dipecah jadi alamat. Satu alamat
  bisa punya beberapa decision, masing-masing dengan masa berlakunya sendiri. `ban` di atas
  `captcha`; captcha belum didukung, jadi diperlakukan seperti ban (fallback standar CrowdSec).
- Kalau LAPI tidak bisa dihubungi, decision yang sudah ada tetap berlaku sampai kedaluwarsa.
  Setelah pull gagal, pull berikutnya mengambil daftar penuh, karena perubahan yang terlewat tidak
  bisa diminta ulang.
- AppSec (WAF) per site (`crowdsec: { appsec: true }`): header request dan body yang panjangnya
  diketahui (≤ `body_limit`, maksimal 64 KiB) dikirim ke komponen AppSec sebelum request
  diteruskan. Body hanya dibaca sekali, lalu diteruskan utuh ke upstream lewat buffer replay
  Pingora. Body yang lebih besar atau chunked diperiksa lewat header saja. Kalau AppSec tidak
  menjawab dalam `timeout` (bawaan 200 ms), request diloloskan atau ditolak sesuai
  `failure_action`.
- Klien yang ditolak mendapat 403 dengan halaman HTML (bisa diganti lewat `ban_page`). Access log
  mencatat `"blocked_by": "crowdsec"` atau `"appsec"` (lainnya: `"rate_limit"`, `"concurrency"`,
  `"mtls"`, `"basic_auth"`, `"forward_auth"`, `"block_exploits:<kategori>"`).
- Parser CrowdSec untuk access log JSON conduit ada di [`deploy/crowdsec/`](../deploy/crowdsec/). Parser
  ini mengisi field yang sama dengan parser nginx, jadi skenario `http-*` bawaan hub langsung
  jalan.

**Upstream pool** (`upstreams:` bernama, atau inline di site)
- Banyak backend dengan bobot. Satu nama DNS yang resolve ke beberapa IP menjadi beberapa backend.
- Algoritma: `round_robin`, `random`, `least_conn` (in-flight dibagi bobot), dan `hash`
  (consistent/Ketama) dengan kunci `client_ip`, `uri`, `path`, `header:<nama>` atau
  `cookie:<nama>`.
- Health check aktif HTTP atau TCP (interval, timeout, `rise`/`fall`, status yang diharapkan,
  TLS/SNI per backend).
- Ejeksi pasif: backend dikeluarkan sementara setelah N error berturut-turut di trafik nyata.
  Kalau semua backend sakit, pool masuk "panic mode" dan tetap mencoba daripada langsung 502.
- Retry ke backend lain saat gagal connect (`retries`, tidak pernah ke backend yang sama dua kali).
- Timeout ke upstream (connect, handshake TLS, baca, tulis) dijawab 504; kegagalan lain (ditolak,
  koneksi putus, respons rusak) 502, sama seperti nginx.
- Pool yang definisinya tidak berubah dibawa saat reload beserta statusnya (sehat, ejeksi,
  in-flight).

**File server** (`files:` per site), seperti `file_server` Caddy
- Site bisa melayani file dari disk saja (tanpa `upstream`), atau campuran: `match` menentukan
  request mana yang diambil dari disk, sisanya ke upstream. `pass_thru: true` meneruskan file
  yang tidak ada ke upstream, bukan 404. `strip_prefix` membuang awalan path sebelum dicari.
- `index` (default `index.html`, `index.txt`), `try_files` untuk single-page app
  (`["{path}", "/index.html"]`), `browse` untuk daftar isi direktori (HTML, di-escape, maksimal
  10.000 entri), `hide` untuk nama yang tidak boleh dilayani.
- Tidak bisa keluar dari `root`: file dibuka dengan `openat2(RESOLVE_BENEATH)`, jadi symlink ke
  luar root ditolak kernel; fallback untuk kernel lama: canonicalize lalu cek awalan. Dotfile
  disembunyikan kecuali `.well-known`. `..` di atas root ditolak saat normalisasi (400). Hanya
  file biasa yang dilayani; FIFO dan device tidak (dibuka `O_NONBLOCK`, jadi tidak bisa membuat
  thread menggantung).
- `ETag` dan `Last-Modified`, request bersyarat sesuai urutan RFC 9110 (`If-Match` →
  `If-Unmodified-Since` → `If-None-Match` → `If-Modified-Since`: 304/412), satu byte range
  (206, 416 dengan `Content-Range: bytes */len`, `If-Range`), HEAD tanpa body.
- URL kanonik: direktori tanpa `/` dan file dengan `/` di-redirect 308, query ikut terbawa.
  `Location` selalu diawali satu `/`, jadi `//host` tidak bisa jadi open redirect.
- `precompressed: [br, zstd, gzip]` mengirim `foo.css.br` dll. bila klien menerimanya, lengkap
  dengan `Vary: Accept-Encoding`. File teks lain dikompres saat dikirim (respons utuh saja).
- File kecil (≤128 KiB) dibaca sekali dan dikirim utuh; file besar dialirkan per 128 KiB dari
  blocking pool, jadi thread request tidak pernah menunggu disk. Content-Type dari tabel ekstensi.
- Site tanpa upstream menjawab method selain GET/HEAD dengan 405 dan `Allow: GET, HEAD`.

**Cache respons** (`cache:` per site, mati secara default; memori bersama di bagian `cache:`)
- Memakai cache HTTP Pingora (`pingora-cache`): penyimpanan di memori dengan eviksi LRU
  berdasarkan ukuran (`max_size`, default 64 MB), objek di atas `max_object_size` (8 MB)
  diteruskan tanpa disimpan. Cache lock: miss bersamaan untuk URL yang sama menunggu satu fetch
  ke upstream (uji: 5 request bersamaan → 1 request ke upstream).
- Aturan cache bersama sesuai RFC 9111: hanya GET/HEAD tanpa body. `private`/`no-store` tidak
  disimpan. Request ber-`Authorization` hanya disimpan kalau `public`/`s-maxage`. `Vary`
  dihormati.
- conduit sendiri tidak pernah menyimpan respons dengan `Set-Cookie`, respons 206, atau
  `Vary: *`.
- `default_ttl` memberi masa segar untuk respons yang tidak menyebutkannya, hanya untuk status
  yang boleh di-cache secara default (200, 301, 404, …).
- Key = site, skema, host, path dan query, ditambah identitas yang conduit kirim ke upstream:
  header dari `forward_auth`/`basic_auth`, fingerprint sertifikat mTLS, dan `X-Forwarded-Host`
  dari proxy tepercaya. Jadi pengguna yang berbeda tidak pernah berbagi entri, dan klien tidak
  bisa meracuni cache lewat header yang diteruskan.
- Respons yang dikompres upstream di-*vary* otomatis menurut `Accept-Encoding` yang sudah
  dinormalisasi. Klien yang tidak menerima gzip tidak pernah menerima entri gzip.
- `stale-while-revalidate`: salinan basi dikirim langsung, lalu diperbarui di background.
- `stale-if-error` (atau `stale_if_error` per site): salinan basi menggantikan saat upstream
  gagal.
- Revalidasi memakai `ETag`/`Last-Modified` (304 dari upstream memperbarui entri).
- Header `Cache-Status` (RFC 9211) di setiap respons site ber-cache, mis. `conduit; hit`,
  `conduit; fwd=miss; stored`, `conduit; hit; detail=stale`.
- Hasilnya juga ada di access log (`"cache"`) dan di metrik `conduit_cache_requests_total{result}`.
- Purge lewat admin: `POST /cache/purge?site=<id>[&prefix=/path]` (JSON jumlah entri). Saat
  reload, entri site yang dihapus atau yang `cache:`-nya dicabut ikut dibuang. Restart dan
  graceful upgrade mulai dengan cache kosong.
- Catatan: API cache Pingora masih ditandai eksperimental oleh Cloudflare. Upstream yang
  menentukan apa yang boleh dibagi: halaman yang berbeda per cookie harus mengirim `private`.

**Plugin** (rantai per site, dijalankan berurutan)
- `rules`: cocokkan method/path/uri/query/scheme/header, lalu `deny`, `redirect`, `respond`
  atau `file`. Path dinormalisasi (`/%5F/`, `//_`, `/a/../_` semuanya jadi `/_/`), jadi encoding
  tidak bisa menembus rule.
- Aksi `proxy` di `rules`: request diteruskan, bukan dijawab. Bisa ke upstream lain (nama pool,
  URL, atau definisi pool; bawaan: upstream site) dan dengan path yang diubah: `strip_prefix`,
  `add_prefix`, atau `rewrite: { regex, to }` (`${1}`, `${nama}`). Ini padanan `location` +
  `proxy_pass` dan `rewrite` di nginx. Perubahan path dihitung dari path yang sudah
  dinormalisasi (jadi `/%61pi/` tetap kena `strip_prefix: /api`), lalu dikirim ter-encode ulang
  bersama query aslinya. Seperti nginx, `%2F` sampai di upstream sebagai `/`; request yang tidak
  diubah path-nya tetap diteruskan apa adanya. Plugin sesudah rule tetap berjalan, dan request
  yang di-route tidak dilayani `files`. Pool inline di rule (`site:<id>:rule<p>.<r>`) ikut health
  check, metrik, dan mempertahankan statusnya saat reload. Entri cache dipisah per pool dan
  target hasil route.
- `body_replace`: ganti string di body respons upstream (padanan `sub_filter`), sambil
  mengalir, tanpa menampung seluruh body. Paling banyak panjang `from` terpanjang dikurangi satu
  byte yang ditahan antar-chunk, jadi pola yang terpotong di batas chunk tetap ketemu. Hanya
  untuk `types` (bawaan `text/html`). Upstream diminta body utuh tanpa kompresi
  (`Accept-Encoding`, `Range`, `If-Range` tidak diteruskan); respons yang tetap terkompresi,
  206, dan jawaban lokal dibiarkan. Kompresi ke klien terjadi sesudahnya. Respons yang diubah
  kehilangan `Content-Length`, `ETag`, dan `Last-Modified`, sama seperti nginx.
- `block_exploits`: tolak request yang membawa payload serangan umum (padanan "Block Exploits"
  NPM), tapi dengan pola yang dibuat supaya request biasa lolos: `redirect_uri=http://...`
  OAuth, pencarian "union station", file repo di `etc/passwd`, `.env.example` di forgejo,
  `/.well-known/`. Yang diperiksa: query (di-decode seperti aplikasi membacanya, `+` jadi
  spasi; di-decode sekali lagi kalau masih ada escape; juga versi tanpa komentar SQL), path
  yang sudah dinormalisasi, `User-Agent`, dan header lain hanya untuk payload header
  (Log4Shell, Shellshock, OGNL). Body tidak dibaca (itu tugas AppSec). Kategori: `traversal`,
  `dotfiles` (`/.env`, `/.git/`, `/.aws/` ... hanya di root), `sqli` (UNION SELECT dengan
  daftar kolom, tautologi `or 1=1` yang dicek kedua sisinya sama, stacked query, time-based,
  error-based), `xss` (tag script, event handler di tag, URL `javascript:` yang diikuti kode),
  `php` (stream wrapper, PHP-CGI, superglobal, object injection, phpunit), `rce` (Log4Shell,
  Shellshock, Spring4Shell, OGNL, template injection, perintah shell, header
  `x-middleware-subrequest` Next.js), `ssrf` (alamat metadata cloud), `scanners` (sqlmap,
  nikto, nuclei, dst. plus daftar NPM tanpa download manager dan TurnitinBot), dan `spam`
  (kata NPM, mati kecuali disebut).
  Tiap rule punya kata kunci literal; satu lintasan Aho-Corasick per input memilih rule yang
  perlu dijalankan, lalu regex (crate `regex`, waktu linear, aman dari ReDoS). Biaya: ±0,2 µs
  untuk request tanpa query, ±0,5–0,7 µs dengan query biasa. `mode: log` hanya menghitung dan
  mencatat. Tiap deteksi dicatat sebagai warning (maks. 20/detik) dengan id rule-nya;
  `disable` mematikan rule tertentu, `skip_params` mengecualikan parameter query (kotak
  pencarian), `except` mengecualikan kategori untuk request yang cocok. Access log mencatat
  `"blocked_by": "block_exploits:<kategori>"`, metrik `conduit_block_exploits_total{site,
  category, action}`. `import-npm` memasangnya di host yang dulu memakai Block Exploits.
- `headers`: set/hapus header request ke upstream dan header respons.
- `ip_filter`: allow/deny CIDR berdasarkan IP klien asli.
- `crowdsec`: tolak klien yang punya decision CrowdSec, dan opsional periksa request lewat AppSec.
- `rate_limit`: maksimal N request per kunci dalam jendela `per` (estimasi geser atas dua
  jendela), sisanya 429 + `Retry-After`. Kunci `client_ip` (IPv6 dihitung per /64), `host`
  atau `header:<nama>`; dengan `match`, hanya request yang cocok yang dihitung dan dibatasi.
- `concurrency`: maksimal N request satu kunci yang sedang berjalan bersamaan; kelebihannya
  langsung ditolak, tidak diantrekan.
- `rate_limit` dan `concurrency` memakai count-min sketch `pingora-limits` (64 KiB per plugin,
  ukuran tetap berapa pun jumlah klien), hitungannya bertahan saat reload selama setelan dan
  posisi plugin tidak berubah, dan bisa melebih-lebihkan tapi tidak pernah meremehkan. Batas
  koneksi sengaja di L7 (request per klien asli), bukan di L4 lewat `ConnectionFilter`: di
  belakang Cloudflare semua koneksi datang dari IP Cloudflare, jadi batas per IP di L4 akan
  mencekik Cloudflare, bukan penyerang.
- `basic_auth`: username/password dengan hash bcrypt (`htpasswd -nB`, `caddy hash-password`),
  dari `users:` atau file htpasswd (di-reload saat berubah), opsional hanya untuk request yang
  cocok `match`. bcrypt dijalankan di blocking pool (maksimal satu per CPU), bukan di thread
  request; password yang sudah terverifikasi diingat sampai reload, jadi request berikutnya
  tanpa hashing. User yang tidak ada tetap diverifikasi terhadap hash lain, supaya waktunya sama.
- `forward_auth`: tanya layanan auth (Authelia, Authentik, oauth2-proxy) lewat GET dengan header
  klien (termasuk cookie) dan `X-Forwarded-Method/-Uri/-Host/-Proto/-For`. 2xx: lolos, header
  `copy_headers` disalin ke request upstream (dibuang kalau tidak ada di jawaban, jadi klien tidak
  bisa memalsukannya). Selain 2xx: jawaban layanan auth dikirim apa adanya ke klien (redirect
  login, beberapa `Set-Cookie`). Tidak menjawab: 502. Body request tidak disentuh. Koneksi ke
  layanan auth di-keep-alive.
- Plugin yang harus menunggu (bcrypt, layanan auth) tetap berjalan sesuai urutan tulisan: rantai
  berhenti di plugin itu, menunggu, lalu lanjut. `rules` setelah `basic_auth` hanya tercapai
  dengan password yang benar. Plugin lain tetap sinkron tanpa alokasi tambahan.

**Observabilitas** (listener `admin.listen`, tanpa autentikasi, taruh di loopback)
- `/metrics` Prometheus: request per site dan kelas status, histogram durasi total dan waktu
  header upstream, byte respons, in-flight, request/kegagalan/status up/ejeksi per backend,
  retry per pool, hasil reload, kedaluwarsa sertifikat, order ACME, jumlah decision CrowdSec,
  hasil pull LAPI, request yang ditolak (decision/AppSec), verdict AppSec, request yang ditolak
  rate limit/concurrency per site (`conduit_limit_refused_total`), hasil `basic_auth` dan
  `forward_auth` per site (`conduit_auth_requests_total{result=allow|deny|error}`), hasil cache
  per site (`conduit_cache_requests_total{result=hit|stale|revalidated|miss|uncacheable|bypass}`),
  isi cache (`conduit_cache_objects`, `conduit_cache_size_bytes`, `conduit_cache_evicted_objects`),
  koneksi upstream idle yang batal dipakai ulang (`conduit_upstream_reuse_lost_total`, lihat
  [Thread dan runtime](#thread-dan-runtime)), log yang dibuang, metrik proses (`process_*`, CPU,
  RSS, file descriptor milik proses yang sedang berjalan).
- `/status` JSON: config, waktu reload terakhir dan error-nya, site (upstream, root `files`, cache,
  plugin, mode mTLS), pool dan status tiap backend, sertifikat, ACME per site, CrowdSec (jumlah
  decision, pull terakhir, error terakhir), isi cache, path access log. `/healthz` untuk liveness.
- `POST /cache/purge?site=<id>[&prefix=/path]` membuang entri cache satu site (semua, atau yang
  path+query-nya berawalan `prefix`, seperti dikirim klien).
- Access log JSON satu baris per request: request ID, site, klien, status, byte, durasi, backend,
  status upstream, waktu connect dan waktu header upstream, jumlah retry, versi TLS. Tiap worker
  menulis ke buffer miliknya sendiri, dan satu thread mengambil buffer itu setiap 20 ms lalu
  menulisnya ke file. Dalam satu jendela itu baris dikelompokkan per worker, jadi urutkan dengan
  `ts`. Kalau buffer penuh, baris dibuang dan dihitung, dan trafik tidak melambat. Mengikuti
  logrotate sendiri.
- Request ID (`X-Request-Id`, 24 hex) di respons, ke upstream, dan di log. ID dari klien hanya
  diterima dari peer tepercaya.

**Operasional**
- Reload tanpa putus: SIGHUP atau perubahan file untuk config dan sertifikat;
  `systemctl reload` (upgrade Pingora, serah-terima socket) untuk binary baru atau alamat listen.
- Renewal yang tertangkap setengah jalan (sertifikat baru, key masih lama) ditolak dan sertifikat
  lama tetap dilayani. Stempel file diambil sebelum dibaca, jadi key yang menyusul memicu reload
  berikutnya.
- `conduit check` memvalidasi config. Key yang tidak dikenal ditolak, jadi typo gagal di `check`.
- `conduit import-npm` mengubah `data/nginx/proxy_host/*.conf` NPM menjadi config conduit.
  Host dengan "Websockets Support" menjadi `upgrade: any`, yang lain `upgrade: off`, sama
  dengan perilaku NPM.

## Target fase 1

| Target | Hasil | Status |
|---|---|---|
| Idle RSS < 30 MB untuk 100 site | 25,6–26,6 MB (build PGO) | ✓ |
| 0 request gagal saat reload, upgrade, dan rotasi sertifikat | 0 gagal (uji di bawah) | ✓ |
| p99 tambahan < 1 ms pada 50% puncak HTTP conduit | Build PGO setelah putaran ketiga patch Pingora, mesin idle, median 3 putaran: +0,51–0,57 ms (3 run di 38.400–39.200 req/s). Setelah putaran pertama +0,60–0,81 ms di 34.700–36.400 req/s; sebelum patch +0,55–0,64 ms di ±28.400 req/s | ✓ |
| Throughput HTTP/1.1 dan HTTPS keep-alive ≥0,75× nginx tuned, atau CPU per request ≤1,4× (diubah 2026-11-08 dari "setara nginx") | HTTP/1.1 0,76–0,81× (CPU 1,31–1,40×), HTTPS 0,77–0,78× (CPU 1,34–1,37×); HTTP/2 dan TLS handshake di atas nginx (1,28–1,32× dan 1,09–1,10×). Lihat [Throughput HTTP/1.1 dan HTTPS](#throughput-http11-dan-https) | ✓ |

### Throughput HTTP/1.1 dan HTTPS

Target fase 1 terakhir, ditutup 2026-11-08. Angka dari build release PGO di mesin idle,
dibandingkan dengan nginx tuned di run yang sama (`scripts/bench.sh dist/conduit`, 2026-11-08).
"Asli" adalah 10 run dengan Pingora asli, "P1" 5 run setelah putaran pertama patch, "P3" 3 run
setelah putaran ketiga:

| Skenario | req/s conduit / nginx: asli | P1 | P3 | CPU per request conduit / nginx: asli | P1 | P3 | Status |
|---|---:|---:|---:|---:|---:|---:|---|
| HTTP/2 | 1,05–1,12× | 1,15–1,22× | 1,29–1,32× | 0,97–1,00× | 0,88–0,90× | 0,80–0,82× | ✓ |
| TLS handshake baru per request | 1,06–1,09× | 1,06–1,08× | 1,10× | 0,87–0,89× | 0,87–0,90× | 0,86–0,89× | ✓ |
| HTTPS keep-alive (HTTP/1.1) | 0,64–0,65× | 0,65–0,71× | 0,77–0,78× | 1,68–1,74× | 1,37–1,56× | 1,34–1,37× | ✓ (≥0,75×) |
| HTTP/1.1 keep-alive | 0,60–0,63× | 0,67–0,70× | 0,76–0,81× | 1,73–1,89× | 1,56–1,61× | 1,31–1,38× | ✓ (≥0,75×) |

Sebelum PGO, HTTP/1.1 keep-alive 0,51–0,55×.

Selisihnya ada di dalam Pingora, bukan di kode conduit. Dari [Profil CPU](#profil-cpu) sebelum
patch: kode conduit ±13% waktu CPU worker, Pingora dan tokio ±51%, syscall 36%. Yang bisa dilakukan dari sisi
conduit sudah dipakai: PGO (+14–16% req/s), `work_stealing: false`, buffer tulis 16 KiB (+11%).
Keputusan 2026-11-07: tetap di Pingora, tidak menulis stack HTTP/1 sendiri.

**Patch Pingora (dikerjakan 2026-11-08).** Pingora 0.9.1 di-fork ke `vendor/pingora` (branch
`conduit`, satu commit per patch) dan dipasang lewat `[patch.crates-io]`. Detail, angka per patch,
dan cara memperbarui ada di [`vendor/README.md`](../vendor/README.md). Ringkasnya:

| Patch | Efek (A/B, build biasa) |
|---|---|
| Koneksi upstream yang idle disimpan tanpa task tokio per koneksi (sebelumnya: satu spawn task, timer, oneshot, dan `Arc<Mutex>` setiap request) | CPU −9% HTTP, −7% HTTPS |
| Cek reuse koneksi memakai alamat yang dicatat saat connect, tanpa `getpeername()` | 1 syscall/request lebih sedikit |
| Cek tipe MIME kompresi dengan prefix, tanpa regex; header hop-by-hop dicek sekali jalan; `insert_header` dengan nama yang sudah di-parse (`CasedHeaderName`) tidak mem-parse ulang | CPU −3–4% |
| Kapitalisasi nama header disimpan sebagai gaya + pengecualian, bukan `HeaderMap` kedua | CPU 0 sampai −1% |
| Satu batch task respons di-flush sekali: respons yang dikompresi keluar dalam 1 `send()`, bukan 3 (header, body, `0\r\n\r\n`) | gzip: req/s +23%, CPU −13,5%, p99 −19% |

Ditambah satu perubahan di conduit sendiri: koneksi upstream di-pool per worker thread (lihat
[Thread dan runtime](#thread-dan-runtime)), HTTP +4% req/s dan CPU −4%.

Release PGO baru dibanding release PGO lama (`scripts/ab.sh`, 7 putaran): HTTP +12% req/s dan
−12% CPU, HTTPS +9% dan −13% CPU, HTTP/2 +9% dan −11% CPU, p99 jenuh −16 sampai −26%; TLS
handshake sama (biayanya di kriptografi). Idle RSS tidak berubah (25,6 MB).

Putaran kedua (patch HTTP/1.1, 2026-11-08 sore), release PGO baru dibanding release PGO pagi itu
(`scripts/ab.sh`, 7 putaran, proxy di 2 core fisik):

| Skenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, jenuh | +6,3% | −8,4% | −3,2% |
| HTTP dengan gzip, jenuh | +26,8% | ±0 | −17,2% |
| HTTPS keep-alive, jenuh | +8,2% | −7,3% | −3,6% |
| HTTP/2, jenuh | +3,0% | +7,0% | −3,0% |
| TLS handshake baru | +3,7% | −3,1% | −1,6% |

Di `scripts/bench.sh` rasio terhadap nginx tuned naik: HTTP 0,66× → 0,71–0,72×, HTTPS 0,70× →
0,72–0,74×, HTTP/2 1,20× → 1,22–1,23×. Gate-nya gagal di satu angka: p99 HTTP jenuh 1,85 ms
(dua run) terhadap baseline 1,30 ms, sementara binary lama di run yang sama 1,19 ms. p50-nya justru
turun (0,96 → 0,79 ms): distribusinya melebar. Di `scripts/ab.sh` dengan tata letak CPU yang sama
selisih p99 jenuh itu +16% dengan rentang per ronde yang tumpang tindih, dan di rate tetap
(31k dan 43k req/s, setara 50% dan 70% puncak) p99 sama dalam batas noise. Baseline belum
direkam ulang.

Putaran ketiga (2026-11-08 malam): kandidat ±1–2% dari profil. Per langkah, build biasa
(`scripts/ab.sh`, 7 putaran):

| Perubahan | HTTP | HTTPS | HTTP/2 |
|---|---|---|---|
| Access log conduit ditulis ulang: baris JSON disusun langsung ke buffer milik tiap worker thread (tanpa `serde_json` dan tanpa channel), thread penulis mengambil semua buffer tiap 20 ms | req/s +1,8%, CPU −2,8% | +2,5%, −2,2% | ±0, −1,8% |
| Header respons upstream diserahkan ke proxy, bukan disalin (`bb3859d`) | +1,4%, −1,4% | +1,7%, −2,2% | −1,8%, −0,9% |
| Channel task HTTP/1 sendiri, menggantikan dua `tokio::sync::mpsc` per request (`c4cce19`; dua A/B berurutan: versi `Mutex`, lalu spinlock) | +0,7%, ±0 | +4,1%, −2,3% | +0,8%, −4,7% |

Penulis JSON access log diuji terhadap `serde_json` dengan 20.000 entri acak (termasuk karakter
kontrol, kutip, dan IPv6), jadi formatnya byte demi byte sama. `scripts/ab.sh` kini juga
memeriksa jumlah baris access log sama dengan jumlah request dan tidak ada yang dibuang.

Clone header request tidak disentuh. Header asli dari klien masih dipakai setelah
`upstream_request_filter` mengubah salinannya: untuk retry ke backend lain, access log, dan key
cache. Menghapus clone itu berarti mengubah kontrak `ProxyHttp`, dan porsinya di profil di bawah
1%.

Release PGO baru dibanding release PGO putaran kedua (`scripts/ab.sh`, 7 putaran, 0 baris access
log hilang):

| Skenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, jenuh | +9,7% | +2,6% | −9,8% |
| HTTP dengan gzip, jenuh | +6,7% | −8,8% | −7,6% |
| HTTPS keep-alive, jenuh | +6,8% | −11,1% | −7,4% |
| HTTP/2, jenuh | +6,0% | −5,9% | −6,2% |
| TLS handshake baru | −2,2% (rentang tumpang tindih) | +3,3% | −1,2% |

Rentang req/s HTTP per putaran tidak tumpang tindih (113–119k terhadap 104–109k).

Di `scripts/bench.sh` (3 run) rasio terhadap nginx tuned naik lagi: HTTP 0,71–0,72× →
0,76–0,81×, HTTPS 0,72–0,74× → 0,77–0,78×, HTTP/2 1,22–1,23× → 1,29–1,32×. 0 baris access log
hilang (±3 juta baris per run). Gate-nya gagal di ketiga run pada p99 skenario 70%: 2,18–3,75 ms
terhadap baseline 1,65 ms. Ini bukan regresi. Rate skenario 70% dihitung dari puncak conduit di
run itu, dan puncaknya naik ±10%, jadi skenario itu kini berjalan di 53.800–54.900 req/s, bukan
±49.000. Di rate itu mesin uji sendiri mendekati jenuh: nginx di run yang sama juga menambah p99
+1,39 sampai +5,97 ms. Pada rate yang sama, release baru lebih baik dari release lama
(`scripts/ab.sh SCENARIOS=rate`, 5 putaran): p99 −13,7% di 49.188 req/s dan −20,6% di 54.901
req/s, CPU −8% dan −9%. Satu run juga mencatat p99 HTTPS jenuh 2,52 ms (dua run lain
1,48–1,49 ms, dan A/B di atas −11%). Sejak itu `scripts/bench.sh` menjalankan skenario 50% dan
70% pada rate dari baseline, bukan dari puncak run itu, sehingga release yang lebih cepat tidak
otomatis diuji di beban yang lebih berat.

Tahap A (2026-11-08 malam), perubahan kecil yang ditunjuk profil per baris: header ke upstream
tanpa `to_string()` untuk alamat klien dan dengan nilai skema statis, counter request ID diambil
per blok oleh tiap worker (bukan satu atomik bersama per request), lookup host dengan foldhash,
dan patch Pingora `0a6aee4` (module HTTP dicari tanpa indeks bersama). A/B build biasa,
7 putaran: HTTP +1,1% req/s (CPU ±0), gzip +2,1% (CPU −1,9%), HTTPS +1,2% (−2,7%), HTTP/2 +3,4%
(−2,9%). Di `scripts/bench.sh` selisih sekecil itu tenggelam dalam noise: HTTP 0,78×, HTTPS
0,77×, HTTP/2 1,28× (gate lulus). Sisa biaya di kode conduit tersebar di banyak titik kecil;
langkah besar berikutnya ada di Pingora.

Kenapa belum setara: setelah putaran ketiga, ±44% CPU worker conduit ada di syscall dan ±56% di
user space. Kalau biaya kernel per request sama dengan nginx (perkiraan, tidak diukur), conduit
memakai sekitar 1,7× CPU user space nginx (sebelum putaran ini ±2,5×). Sisa biayanya tersebar
tipis tanpa satu titik di atas ±1,5% ([Profil CPU](#profil-cpu)): alokasi, memcpy future async
yang besar di antara `await`, dan kode perekat proxy Pingora. Langkah berikutnya bukan lagi
tambalan kecil, tetapi perubahan bentuk alur request di Pingora (future yang lebih kecil, lebih
sedikit alokasi per request).

Target awalnya "setara nginx tuned". Keputusan 2026-11-08: target diubah menjadi ≥0,75× nginx
tuned atau CPU per request ≤1,4× (cukup salah satu), tanpa melanggar target p99 dan idle RSS di
atas, dan fase 1 ditutup. Alasannya:
- Target lain sudah tercapai.
- Di host produksi selisih ini tidak membatasi apa pun: trafik < 1.000 req/s, sedangkan conduit di
  2 CPU sanggup ±78.000 req/s HTTP dan ±58.000 req/s HTTPS.
- Langkah berikutnya bukan lagi tambalan kecil: jalur cepat HTTP/1 di fork Pingora (perkiraan
  ±0,9–0,95×, diff fork besar dan sulit di-rebase), dan untuk 1,0× kemungkinan stack HTTP/1
  sendiri, yang bertentangan dengan keputusan 2026-11-07.

Jalur cepat itu tetap terbuka sebagai pekerjaan opsional kalau setara nginx kelak dibutuhkan.

## Hasil uji (lokal)

| | |
|---|---|
| Unit test | 114 lulus (plus 2 pengukuran, `--ignored`) |
| Header `Server` (`server_header` e2e) | `server_header: conduit`: respons proxy (menggantikan `Server: echo-upstream`), 404 host tak dikenal, 403 `ip_filter`, 502 upstream mati, 400 header rusak, dan 405 `CONNECT` semuanya membawa tepat satu `Server: conduit`; site dengan `headers` remove tanpa `Server`. Setelah reload tanpa opsi: `Server` upstream diteruskan, dan kelima respons lokal tadi tanpa `Server` (bukan `Pingora`). Nilai berisi CRLF ditolak saat reload dan config lama tetap jalan. Unit test: kosong, spasi di tepi, non-ASCII, lebih dari 64 karakter → ditolak |
| `block_exploits` di trafik nyata (2026-11-09) | 1.139.833 request dari access log NPM produksi (17 Apr–9 Nov 2026, 51 proxy host; dibaca read-only, IP klien di-hash sebelum disimpan, salinan dihapus setelahnya) diputar ulang ke engine dengan setting bawaan. 25.117 (2,2%) akan diblokir, dan semuanya diperiksa: probe file rahasia (`/.env*`, `/.git/config`, `/.aws/`, `.npmrc`, dst.; 22.554, dari 2.313 klien), traversal (`../../`, `..;/`, `/proc/self/environ`), phpunit `eval-stdin.php`, OGNL Struts, Shellshock, pearcmd, metadata cloud lewat parameter proxy, `?=phpinfo()`, UA zgrab, dan Dolibarr `file=../conf/conf.php`. Satu salah blokir ditemukan dan diperbaiki: TurnitinBot (crawler sah, dari daftar UA NPM; download manager di daftar itu ikut dibuang). Satu serangan yang lolos ditambahkan: naik satu tingkat ke file config (Dolibarr, 145 request ke 7 host). Setelah perbaikan, selisih dengan run sebelumnya hanya dua hal itu. Yang diblokir NPM tapi diloloskan conduit: `=http://` (disengaja, lihat paritas) dan `/./`; sisanya berasal dari rule header Next.js di advanced config, yang di conduit ditangani `rules` |
| `block_exploits` (unit + `block_exploits` e2e) | 163 payload serangan (sqlmap, nuclei, PayloadsAllTheThings, Log4Shell dan obfuscation-nya, Spring4Shell, OGNL Confluence/Struts, PHP-CGI CVE-2024-4577, phpunit, ThinkPHP, pearcmd, reverse shell, metadata cloud, UA scanner, Dolibarr `../conf/conf.php`) harus kena rule yang tepat; 74 request biasa yang mirip serangan harus lolos (OAuth `redirect_uri=http://`, URL presigned S3, digest registry, file repo `etc/passwd` dan `.env.example`, pencarian kode `select * from`, `time.sleep(5)`, `<button onclick=`, "JavaScript: The Good Parts", "O'Reilly and Sons", `order=desc`, variabel Grafana `${env:raw}`, WebDAV `.git/`, `/.well-known/`, UA Uptime Kuma/curl/python-requests/TurnitinBot/GetRight, `../` ke file biasa). Hasil dengan dan tanpa kata kunci Aho-Corasick sama untuk semua input itu. E2E: serangan → 403 dan tidak sampai upstream, request biasa sampai utuh, `except` dan `skip_params`, mode `log` meloloskan dan menghitung, metrik per kategori, `blocked_by` di access log, baris warning dengan id rule. Biaya (`cost`, release): ±0,22 µs tanpa query, ±0,5 µs query biasa, ±0,6 µs URL OAuth, ±19 µs query 5 KB |
| Route dan `body_replace` (`routes_and_body_replace`) | `strip_prefix` ke pool bernama, termasuk path ter-encode (`/%61pi/./v1/../v2` → `/v2`) dan spasi yang di-encode ulang, `rewrite` regex dengan query tetap, `add_prefix` ke URL inline berdasarkan header, request tanpa route diteruskan apa adanya; rule `deny` sebelum route tetap menang, dan plugin sesudah route tetap berjalan. `body_replace`: pola yang terpotong di 3 chunk upstream tetap diganti, upstream tidak menerima `Accept-Encoding`/`Range`, `ETag`/`Last-Modified`/`Content-Length` dibuang, respons ber-`Content-Length` jadi chunked, klien HTTP/1.0 menerima body sampai koneksi ditutup, JSON dan body gzip dibiarkan. Unit test: setiap cara memotong body (dua potongan di mana saja, dan per byte) memberi hasil sama dengan body utuh |
| End-to-end (binary asli) | proxy dan header, request ID, SNI, HTTP/2, plugin, 502, sebaran LB ke 2 backend, health check menandai backend mati, retry + ejeksi pasif, metrik, endpoint admin, reload mempertahankan status pool, reload gagal tercatat di `/status`, access log JSON, rate limit (429 + `Retry-After` hanya di path yang cocok dan per klien, hitungan bertahan saat reload) dan concurrency (request ketiga ditolak seketika saat dua masih berjalan, klien lain tidak terpengaruh), body request dan respons (0 B sampai 3 MB naik dan 20 MB turun, `Content-Length` dan chunked dengan potongan tak rata, upstream yang membaca lambat, satu koneksi keep-alive untuk semuanya, 8 klien paralel; isi dicek dengan SHA-256) |
| mTLS (handshake asli) | tanpa sertifikat, CA lain, dan sertifikat kedaluwarsa → 403 dengan alasannya; sertifikat sah → upstream menerima subject dan fingerprint, header palsu dari klien dibuang; `optional`; SNI ≠ Host → 421, site dengan CA sama saling menerima; HTTP/2 lewat curl; **resumption TLS 1.2 dan 1.3**: sesi site yang sama di-resume dengan sertifikatnya, sesi dari site tanpa mTLS atau CA lain tidak di-resume, sesi dengan sertifikat buruk tetap ditolak; file CA diganti → sesi lama tidak di-resume dan CA baru langsung berlaku. Lulus 5 dari 5 kali |
| Auth | `basic_auth` hanya di path yang cocok, rule setelahnya tidak tercapai tanpa password, password salah dan user tak dikenal → 401, verifikasi berikutnya dari cache; `forward_auth`: tanpa sesi → redirect login dengan 2 `Set-Cookie` utuh, header `X-Forwarded-*` yang diterima layanan auth benar dan tidak bisa dipalsukan klien, `copy_headers` disalin atau dibuang, body POST sampai utuh, layanan auth mati → 502; metrik dan `blocked_by`. Lulus 5 dari 5 kali |
| Rotasi sertifikat dan reload di bawah beban | trafik HTTP dan HTTPS terus-menerus (±6.000 + ±600 request) selama: rotasi gaya certbot (rename, key dulu), sertifikat ditulis di tempat dengan key menyusul 1,5 detik kemudian (reload setengah jalan ditolak, sertifikat lama tetap dilayani), dan site baru lewat SIGHUP. 0 gagal, kedua sertifikat baru terlihat, koneksi TLS yang dibuka sebelum rotasi tetap jalan. Lulus 5 dari 5 kali |
| Graceful upgrade | ribuan request selama serah-terima socket, 0 gagal; `/status` menunjukkan PID baru |
| Graceful upgrade dengan WebSocket terbuka (`upgrade_with_open_websockets`) | 5 WebSocket dibuka ke proses lama, lalu `run -u` + SIGQUIT: WebSocket lama tetap bolak-balik selama serah-terima dan masa grace; ±5,5 detik setelah SIGQUIT koneksi baru dan `/status` sudah dilayani proses baru; proses lama keluar 7–12 detik setelah SIGQUIT (5 detik Pingora + `shutdown_grace` + `shutdown_timeout`) dan memutus WebSocket yang masih dipegangnya; setiap WebSocket itu tetap punya baris access log (101, `"error":"Cancelled"`); WebSocket di proses baru jalan terus |
| Klien idle, lambat, dan rusak (`idle_and_malformed_clients`, `slow_request_head_is_cut_off`) | koneksi keep-alive idle ditutup sesuai `keepalive_timeout` walau klien minta `Keep-Alive: timeout=600`; header 64 KiB → 200; header 2 MiB, 300 baris header, dan sampah → 4xx tanpa menggantung; header yang dikirim satu byte per beberapa detik diputus setelah 62 detik (`--ignored`, butuh ±1 menit) |
| fd habis (`file_descriptors_run_out`) | conduit dengan `ulimit -n 128` dibanjiri 250 koneksi: tidak panic, tidak berputar (CPU < 500 ms dalam 3 detik saat accept gagal `EMFILE`), dan melayani lagi begitu koneksi ditutup |
| Fuzzing (`fuzz/`, cargo-fuzz + libFuzzer) | 8 target (`config`, `import`, `path`, `access_log`, `request`, `route`, `body_replace`, `block_exploits`; lihat [Perintah](#perintah)). Setelah `block_exploits` ditambahkan (juga dipasang di harness `request`), 10 menit tiap target terdampak: 1,26 juta input `block_exploits` (232 seed dari korpus serangan), 4,0 juta request, 0,72 juta config, 0,80 juta file NPM, 0 crash dan 0 beda antara jalur cepat dan jalur tanpa kata kunci. 20 menit tiap target setelah perbaikan: 1,4 juta config, 1,5 juta file NPM, 101 juta path, 17 juta baris access log, 8,9 juta request, 0 crash. Setelah aksi `proxy` dan `body_replace` ditambahkan, 10 menit lagi untuk tiap target (config, import, path, access_log: 0,59 / 0,59 / 41 / 6,5 juta), lalu 10 menit dengan route dan `body_replace` di harness `request`: 5,2 juta request, 4,5 juta target route, 2,8 juta body (dua putaran), 0 crash. Satu temuan di `route` ternyata asumsi harness yang salah: query berisi `<` sudah ditolak Pingora (400) sebelum sampai ke route; input itu kini jadi seed. Tiga temuan sebelumnya sudah diperbaiki dan punya unit test: `import-npm` dengan karakter kontrol (`control_characters_stay_inert`), tanpa sertifikat (`no_certificates_still_loads`), dan target tanpa path (`target_without_path_is_400`) |
| Upstream flapping di bawah beban (`upstream_flapping`) | ±100 ribu request GET dan POST dari 6 klien (keep-alive dan koneksi baru) ke pool 3 backend, selama: satu backend mati 2,5 detik, satu backend hang 2,5 detik, ketiganya mati 1,5 detik, lalu satu backend mati-hidup 20 kali tiap 100 ms. Setiap request yang gagal harus terjelaskan: 502 hanya untuk request yang sedang berjalan saat backend mati, atau selama semua backend mati (gagal seketika, di bawah 100 ms, tidak menumpuk); 504 hanya untuk backend yang hang (4–6 request sebelum health check mengeluarkannya). GET di koneksi upstream yang ternyata sudah putus di-retry otomatis, POST tidak (tidak aman diulang). Tidak ada jawaban tertukar, tidak ada koneksi klien yang diputus tanpa jawaban, dan tidak ada panic. Setelahnya semua backend kembali menerima trafik, `in_flight` kembali 0, jumlah fd kembali seperti awal, dan setiap request tercatat sekali di access log. Lulus 4 dari 4 kali |
| Paritas dengan NPM (`scripts/parity/run.sh`, lokal) | nginx dari image NPM 2.14.0 (versi yang sama dengan prod) memuat salinan config 51 proxy host, conduit memuat hasil `import-npm` dari salinan yang sama plus terjemahan 9 advanced config; keduanya ke upstream tiruan yang menjawab dengan request yang diterimanya. 189 request per proxy: `/` lewat HTTP dan HTTPS dengan UA browser dan non-browser di semua host, ditambah semua aturan advanced (redirect forgejo khusus browser, CORS + `OPTIONS` 204, blok header Next.js, `debug.log` 404, `mobile-config.json`, konsol S3, blok admin PocketBase termasuk `%5F` dan huruf besar), path dengan `..`/spasi/encoding, POST, upgrade WebSocket dan headscale, host tak dikenal. Status, `Location`, `Content-Type`, header CORS/cache/HSTS, dan target/metode/header yang sampai ke upstream sama. Beda yang disengaja: conduit menambah `X-Forwarded-Host` dan `X-Request-Id`, memakai koneksi upstream keep-alive (NPM `Connection: close`), tidak mengirim `X-Forwarded-Scheme`, tidak mempercayai `X-Forwarded-Proto` dari klien sembarang (NPM meneruskannya), dan mengompres JSON untuk klien yang meminta gzip. Sejak `block_exploits` diimpor (38 host): 197 request per proxy, dan beda status hanya yang disengaja. NPM memblokir tapi conduit meneruskan: `redirect_uri=http://...`, `<div>description</div>` (pola NPM `<.*script.*>`), `concat(`. NPM meneruskan tapi conduit memblokir: `' union select 1` (pola NPM butuh `(`), `/.env`, `/..;/`, Log4Shell di header, UA sqlmap, `' or '1'='1`. Serangan yang diblokir NPM (`union select (1)`, `<script>`, `../../etc/passwd`) diblokir keduanya |
| ACME dengan Pebble (CA uji Let's Encrypt, `mise run test-acme`) | terbit ±1 detik setelah start; restart memakai sertifikat tersimpan tanpa order baru; sertifikat yang hampir kedaluwarsa diperpanjang di tengah ±285 request HTTPS dengan 0 gagal; nama yang ditolak CA tercatat di `/status` dan dicoba lagi nanti tanpa mengganggu site lain. Lulus 5 dari 5 kali (termasuk setelah perubahan fase 2 terakhir) |
| CrowdSec asli v1.8.2 di Docker (`mise run test-crowdsec`) | decision `cscli` berlaku di conduit ±1 detik kemudian (alamat, range, captcha → ditolak), dihapus → lolos lagi; AppSec menolak `/.env` dan body berisi pola terlarang, body bersih sampai utuh ke upstream, site tanpa AppSec tidak terpengaruh; CrowdSec di-pause: AppSec lolos setelah timeout, decision yang ada tetap berlaku, pull pulih otomatis; **loop penuh**: 20 request probing 404 di access log → parser `deploy/crowdsec` → skenario `http-probing` → IP diblokir conduit ±2 detik setelah probe terakhir. Lulus 5 dari 5 kali (termasuk setelah perubahan fase 2 terakhir) |
| File server | index, `ETag`/`Last-Modified` → 304, `If-Match` → 412, range 206 tanpa kompresi, 416 dengan `Content-Range`, `If-Range` usang → 200, file 300 KB dialirkan utuh, gzip untuk file teks utuh, HEAD, precompressed `.br` + `Vary`, redirect kanonik 308 dengan query (`//sub` → `/sub/`, tidak pernah ke host lain), dotfile/`%2e`/symlink ke luar root → 404, `..` di atas root → 400, POST ke site tanpa upstream → 405, `try_files` SPA, campuran file + upstream dengan `strip_prefix` dan `pass_thru`, `/status` |
| Cache respons | miss → hit (`Cache-Status`), query dan site masuk key, `X-Forwarded-Host`/`-Proto` dari peer tepercaya masuk key, `Set-Cookie`/`private`/tanpa info freshness/`Authorization` tidak disimpan, POST → bypass, `default_ttl`, `Vary`, objek > `max_object_size` diteruskan utuh tanpa disimpan, cache lock (5 bersamaan → 1 ke upstream), `stale-while-revalidate` (basi lalu diperbarui di background), `stale-if-error` saat upstream mati, purge per prefix dan per site lewat admin (dan validasi parameternya), metrik, `/status`, access log |
| WebSocket dan upgrade | WebSocket 101 dengan `Sec-WebSocket-*` sampai upstream, 8 KiB bolak-balik di site dengan `max_body_size: 1k`; upgrade gaya headscale (`tailscale-control-protocol`, `X-Tailscale-Handshake`) di `upgrade: any` sampai utuh tanpa `Keep-Alive`/`TE`, dan di mode default sampai sebagai request biasa; `upgrade: off` menolak WebSocket; `Upgrade` palsu dengan body 4 KiB tetap 413; koneksi diam ditutup ±2 detik sesuai `upgrade_idle_timeout` atau `read_timeout`, dan tetap terbuka selama ada data; site dengan cache; reload tidak memutus koneksi terbuka; access log 101 dengan byte dan tanpa error. Dicek juga dengan mutasi: tanpa perbaikan, test gagal. Lulus 4 dari 4 kali |
| Fork Pingora (`vendor/pingora`) | test unit `pingora-http` (58 + 3 doctest), `pingora-proxy` (65), dan `pingora-core` (628, dengan OpenSSL), termasuk test baru untuk pool idle, batas waktu seluruh header request (slowloris), cek reuse, cek MIME, hop-by-hop, `CasedHeaderName`, header `Server` buatan Pingora, serah-terima header respons (9 bentuk respons, byte dan keep-alive dibandingkan dengan jalur lama), dan channel task HTTP/1 (urutan, menunggu slot, akhir aliran, penerima hilang, 10.000 nilai antar thread). Semua test conduit di atas (86 unit, 9 e2e, ACME, CrowdSec) diulang sekali dengan fork dan lulus |
| clippy `-D warnings` | bersih |

## Benchmark

`scripts/bench.sh`: proxy dibatasi 2 CPU seperti host produksi, upstream dan load generator di CPU
lain, body 2 KB, 64 koneksi, 50.000 request per skenario, access log aktif di kedua proxy.

Satu run (2026-11-08 malam, release PGO setelah putaran ketiga patch Pingora, mesin idle; run
tengah dari 3). Rentang ada di [Throughput HTTP/1.1 dan HTTPS](#throughput-http11-dan-https). Baseline
gate masih dari release putaran pertama.

| Skenario | Proxy | req/s | p99 | CPU per 10k req | RSS puncak |
|---|---|---:|---:|---:|---:|
| HTTP keep-alive | nginx gaya NPM | 35.772 | 1,96 ms | 0,34 s | 19 MB |
| | nginx tuned | 98.917 | 0,77 ms | 0,15 s | 19 MB |
| | **conduit** | **77.006** | 1,16 ms | 0,20 s | 30 MB |
| HTTPS (HTTP/1.1) | nginx tuned | 74.518 | 0,95 ms | 0,21 s | 29 MB |
| | **conduit** | **57.982** | 2,52 ms¹ | 0,28 s | 33 MB |
| HTTP/2 | nginx tuned | 37.956 | 1,84 ms | 0,41 s | 29 MB |
| | **conduit** | **49.784** | 1,99 ms | 0,33 s | 36 MB |
| TLS handshake baru per request | nginx tuned | 3.277 | 20,65 ms | 5,76 s | 30 MB |
| | **conduit** | **3.601** | 33,82 ms | 4,96 s | 37 MB |

¹ Dua run lain 1,48–1,49 ms.

Idle RSS dengan 100 site, 2 host per site, plugin di setiap site, dan 10 pool dengan health
check: **25,6–26,6 MB** dengan build release PGO (1 site: 21,4–22,6 MB), termasuk service ACME
yang menganggur. Angka atas dari pengukuran ulang 2026-11-08 dengan binary rilis terakhir (sepuluh
patch fork, tahap A, access log baru). Build tanpa PGO: 26,8–27,3 MB. Parser YAML serde-saphyr menambah ±0,8 MB
dibanding serde_yaml (diukur dengan config dan binary pembanding yang sama).

Dengan CrowdSec (20 site, LAPI tiruan berisi 70.200 decision, JSON 15,9 MB): idle RSS 19,6 MB
tanpa CrowdSec, **27,4 MB** dengan 70.200 decision. Sesaat setelah pull penuh sempat 44 MB
(respons JSON di memori), lalu turun sendiri.

Cara membaca angka ini:
- conduit **±2,2× NPM** (req/s 2,15–2,18×, p99 dan CPU lebih rendah), **lebih cepat dari nginx
  tuned di HTTP/2** (1,29–1,32× req/s, CPU 0,80–0,82×), dan **lebih cepat di TLS handshake**
  (1,10× req/s, CPU 0,86–0,89×).
- Di request keep-alive kecil HTTP/1.1 dan HTTPS, conduit 0,76–0,81× throughput nginx tuned.
  Profil samply (lihat di bawah) menunjukkan selisih ini hampir seluruhnya ada di Pingora.
- Di host produksi, overhead ini tidak terasa: trafik jauh di bawah 1.000 req/s, dan latensi proxy
  keduanya di bawah 1 ms per request. Yang penting di sana adalah memori: container NPM memakai
  ±364 MB.

### Perbandingan dengan reverse proxy lain

`scripts/compare.py` (`mise run compare`) menjalankan nginx, Caddy, Apache httpd, HAProxy,
Traefik, Envoy, dan conduit dengan beban dan setup yang sama:
- Setiap proxy jalan di container dengan 2 CPU yang sama.
- Semua proxy ke upstream yang sama, body 2 KB, 64 koneksi, 50.000 request per run.
- 3 putaran dengan urutan proxy bergantian; tabel memakai median.
- CPU dan memori diambil dari cgroup container. conduit juga dijalankan di container
  (`debian:trixie-slim` dengan `dist/conduit`), supaya cara ukurnya sama.

Tabel di bawah diambil sebelum putaran patch HTTP/1.1 kedua (lihat
[Throughput HTTP/1.1 dan HTTPS](#throughput-http11-dan-https)); belum dijalankan ulang.

Setiap proxy dikonfigurasi seperti untuk produksi:
- 2 worker atau thread.
- Keep-alive ke upstream. Dicek dari jumlah koneksi baru yang diterima upstream: paling banyak
  74 per run (nginx tuned) dari 50.000 request. nginx gaya NPM membuka satu koneksi per request.
- TLS dengan sertifikat ECDSA P-256 yang sama, dan HTTP/2 di TLS.
- `X-Forwarded-For` dan `X-Forwarded-Proto` ke upstream.
- Access log setiap request ke file. Dicek dari jumlah barisnya: lengkap di semua proxy, kecuali
  HAProxy yang kehilangan 0,2% baris (HAProxy membuang baris log saat sibuk) dan Apache yang
  masih menahan 19 baris di buffer saat dihitung.

Versi: nginx 1.28, Caddy 2.11.9, Apache 2.4.69 (event MPM), HAProxy 3.4.7, Traefik 3.7.17,
Envoy 1.40.2 (BoringSSL), dan conduit release PGO dengan patch Pingora. Diukur 2026-11-08 di
mesin idle, 2 run. Tabel di bawah dari run kedua. Rasio terhadap nginx di run pertama berbeda
paling banyak 0,02.

req/s, dengan rasio terhadap nginx tuned dalam kurung:

| Proxy | HTTP/1.1 keep-alive | HTTPS keep-alive | HTTP/2 | Koneksi TLS baru per request |
|---|---:|---:|---:|---:|
| nginx tuned | 101.079 | 74.070 | 37.906 | 3.313 |
| HAProxy | 75.048 (0,74×) | 56.834 (0,77×) | **54.092 (1,43×)** | 2.932 (0,88×) |
| **conduit** | **69.776 (0,69×)** | **51.323 (0,69×)** | **43.983 (1,16×)** | **3.510 (1,06×)** |
| nginx gaya NPM | 35.199 (0,35×) | | | |
| Apache httpd | 33.689 (0,33×) | 25.426 (0,34×) | 13.686 (0,36×) | 2.183 (0,66×) |
| Envoy | 33.389 (0,33×) | 30.344 (0,41×) | 26.912 (0,71×) | **5.178 (1,56×)** |
| Caddy | 18.457 (0,18×) | 16.457 (0,22×) | 12.298 (0,32×) | 3.271 (0,99×) |
| Traefik | 17.596 (0,17×) | 15.182 (0,20×) | 11.139 (0,29×) | 3.283 (0,99×) |

CPU per 10.000 request, dan memori:

| Proxy | HTTP/1.1 | HTTPS | HTTP/2 | TLS baru | p99 tambahan di 8.800 req/s | Memori idle | Memori puncak |
|---|---:|---:|---:|---:|---:|---:|---:|
| nginx tuned | 0,15 s | 0,20 s | 0,41 s | 5,68 s | +0,09 ms | 9,1 MB | 10–13 MB |
| HAProxy | 0,21 s | 0,28 s | 0,30 s | 6,40 s | +0,08 ms | 19,6 MB | 22–26 MB |
| **conduit** | 0,23 s | 0,30 s | 0,35 s | 5,19 s | +0,12 ms | 9,1 MB | 23–26 MB |
| nginx gaya NPM | 0,35 s | | | | +0,44 ms | 8,7 MB | 9 MB |
| Apache httpd | 0,53 s | 0,70 s | 1,34 s | 8,66 s | +0,39 ms | 6,0 MB | 38–47 MB |
| Envoy | 0,54 s | 0,58 s | 0,66 s | 3,53 s | +0,44 ms | 14,3 MB | 21 MB |
| Caddy | 1,00 s | 1,11 s | 1,47 s | 5,75 s | +2,85 ms | 9,7 MB | 28–29 MB |
| Traefik | 1,06 s | 1,22 s | 1,64 s | 5,73 s | +3,24 ms | 13,5 MB | 44–47 MB |

"p99 tambahan" diukur di rate tetap 8.800 req/s selama 10 detik, 50% dari puncak HTTP proxy
paling lambat (Traefik), jadi tidak ada proxy yang jenuh. Angkanya adalah p99 lewat proxy
dikurangi p99 langsung ke upstream (0,28 ms). Memori di sini adalah memori anonim cgroup (heap
dan stack, tanpa halaman binary), jadi lebih kecil dari RSS di bagian lain README ini.

Cara membaca:
- conduit masuk kelompok tercepat bersama nginx dan HAProxy. Di HTTP/1.1 conduit ±2× Apache dan
  Envoy, dan ±3,8× Caddy dan Traefik.
- HTTP/1.1 dan HTTPS: nginx tercepat, HAProxy 0,74–0,77×, conduit 0,69×. Penyebab selisih
  conduit dibahas di [Throughput HTTP/1.1 dan HTTPS](#throughput-http11-dan-https).
- HTTP/2: HAProxy tercepat (1,43× nginx), conduit kedua (1,16×), nginx ketiga.
- Koneksi TLS baru: Envoy jauh di depan (1,56×), kemungkinan karena BoringSSL; ini tidak diukur
  lebih jauh. conduit 1,06×; nginx, Caddy, dan Traefik setara; HAProxy 0,88×; Apache 0,66×.
- Latensi di beban sedang: nginx, HAProxy, dan conduit menambah ≤0,12 ms p99. Apache, Envoy, dan
  nginx gaya NPM sekitar 0,4 ms. Caddy dan Traefik (Go) menambah ±3 ms.
- Memori idle semua proxy kecil (6–20 MB). Saat beban, Apache dan Traefik naik paling tinggi.
- Batasan: ini beban sintetis (body 2 KB, localhost, 2 CPU). Konfigurasinya wajar untuk
  produksi tapi tidak di-tuning habis untuk setiap proxy. Contohnya, Caddy dan Traefik memakai
  format log JSON bawaan yang mencatat semua header, dan rotasi log Caddy dimatikan seperti
  proxy lain. Di host produksi, dengan trafik jauh di bawah 1.000 req/s, semua proxy ini cukup cepat.

### Latensi di bawah beban

Rate tetap selama 10 detik (`oha -q`, `--latency-correction`): latensi dihitung dari jadwal kirim
tiap request, jadi waktu antre ikut terukur. Overhead = p99 lewat proxy dikurangi p99 langsung
ke upstream. Angka dari `scripts/bench.sh` (median 3 putaran per run, release PGO): 5 run
setelah putaran pertama patch Pingora (P1) dan 3 run setelah putaran ketiga (P3). Rate-nya ikut
puncak conduit di run itu, jadi release yang lebih cepat diuji di rate yang lebih tinggi:

| Beban (dari puncak HTTP conduit) | Release | req/s | p99 upstream langsung | + nginx tuned | + conduit |
|---|---|---:|---:|---:|---:|
| 50% | P1 | 34.700–36.400 | 0,48–0,50 ms | +0,36–0,49 ms | **+0,60–0,81 ms** |
| | P3 | 38.400–39.200 | 0,52–0,53 ms | +0,49–0,64 ms | **+0,51–0,57 ms** |
| 70% | P1 | 48.600–50.900 | 0,60–0,61 ms | +0,70–0,82 ms | +1,03–1,17 ms |
| | P3 | 53.800–54.900 | 0,67–0,68 ms | +1,39–5,97 ms | +1,50–3,09 ms |

Di P1, angka tertinggi conduit (+0,81 dan +1,17 ms) dari run dengan rate tertinggi (36.400 dan
50.900 req/s); empat run lain +0,60–0,64 ms dan +1,03–1,06 ms. Di P3, 70% puncak sudah dekat batas
mesin uji (proxy, upstream, dan load generator berbagi 8 CPU), dan p99 kedua proxy melebar dan
berubah-ubah antar run. Pada rate yang sama, P3 lebih baik dari release putaran kedua: lihat
[Throughput HTTP/1.1 dan HTTPS](#throughput-http11-dan-https).

Sebelum patch, di rate yang lebih rendah (±28.400 dan ±39.800 req/s), conduit menambah
+0,79–0,85 ms dan +1,41–1,42 ms (satu jendela 10 detik, bukan median).

- Pada rate absolut yang sama, nginx menambah lebih sedikit, karena rate itu baru seperempat
  sampai sepertiga kapasitasnya.
- Pada beban relatif yang sama (persentase puncak masing-masing, diukur terpisah), conduit lebih
  stabil. nginx tuned di 50/60/70% puncaknya menambah p99 +2,99 / +1,05 / +12 ms (p99.9 ±33 ms
  di 70%). conduit di 49/60/69% menambah +0,69 / +1,05 / +1,3–1,4 ms. Access log hanya ±0,1 ms
  dari angka itu.
- Trafik host produksi (< 1.000 req/s) kira-kira 2% dari puncak. Di 9% puncak conduit menambah
  +0,11 ms.

Regression gate `scripts/bench.sh` gagal kalau:
- ada request yang gagal;
- conduit menambah p99 lebih dari `P99_OVERHEAD_MS` (default 1 ms) di skenario 50%;
- req/s conduit, dihitung sebagai rasio terhadap nginx tuned di run yang sama, turun lebih dari
  `TOLERANCE` (default 10%) dari `target/bench/baseline.json`;
- p99 conduit naik lebih dari `P99_TOLERANCE` (default 25%) dan lebih dari 0,2 ms dari baseline.
  Skenario 70% dijaga lewat aturan ini, dan untuk skenario rate tetap yang dibandingkan hanya p99.
  Kalau ada baseline, skenario 50% dan 70% memakai rate dari baseline, jadi p99 dibandingkan pada
  beban yang sama. Tanpa baseline, atau saat merekamnya, rate-nya 50% dan 70% puncak run itu.

Skenario rate tetap (50% dan 70%) diulang `RATE_ROUNDS` kali (default 3), bergantian upstream,
nginx, conduit, dan yang dinilai adalah mediannya. Satu jendela 10 detik mudah terkena gangguan
sesaat dari desktop: di 2 dari 6 run nginx dan conduit melonjak bersamaan (nginx 2,4 ms dan
10,9 ms, padahal biasanya ±1,0 ms).

Kenapa rasio dan toleransi p99 terpisah: di mesin idle, tiga run berturut-turut dengan binary
yang sama menurunkan req/s nginx dan conduit bersama-sama ±5% (mesin makin panas), sedangkan
rasionya tetap (HTTP/1.1 0,60–0,62×, HTTPS 0,64×). p99 saat jenuh bervariasi sampai 15% antar
run (HTTP/2 2,74–3,16 ms). Dengan aturan lama (angka absolut, 10%) binary yang sama gagal
melawan baseline-nya sendiri.

`UPDATE_BASELINE=1` menyimpan baseline baru. Rekam di mesin idle dengan build release (PGO):
`UPDATE_BASELINE=1 scripts/bench.sh dist/conduit`.

### Thread dan runtime

`server.work_stealing: false` (default) memberi setiap worker thread runtime Tokio sendiri. Koneksi
diserahkan ke salah satu thread saat accept dan tidak pernah pindah, sehingga tidak ada antrean
bersama atau pencurian task antar-thread. `true` memakai satu runtime yang thread-nya saling
mencuri task. Mode ini meratakan beberapa koneksi berat yang hidup lama (HTTP/2, WebSocket),
dengan biaya throughput dan latensi untuk banyak koneksi biasa.

A/B `scripts/ab.sh` (7 ronde bergantian, median, proxy di 2 core fisik). Angkanya adalah
perubahan `false` terhadap `true`:

| Skenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, jenuh | +5,9% | +8,8% | −8,2% |
| HTTPS keep-alive, jenuh | +4,7% | +14,4% | −4,1% |
| HTTP/2, jenuh | −2,0% | +25,9% | +1,5% |
| TLS handshake baru, jenuh | −3,6% | +22,9% | −1,8% |
| HTTP rate tetap ±50% puncak (35k req/s) | sama | **−6,4%** | −7,6% |
| HTTP rate tetap ±70% puncak (50k req/s) | sama | **−47,8%** (6,98 → 3,65 ms) | −5,1% |

Host produksi berada di belakang Cloudflare, dan trafik dari Cloudflare ke origin hampir seluruhnya
HTTP/1.1 keep-alive, jauh di bawah titik jenuh. Di kondisi itu `false` lebih hemat CPU dan p99-nya
jauh lebih stabil. p99 di beban jenuh memang lebih buruk, karena thread yang kebetulan dapat lebih
banyak koneksi tidak dibantu thread lain.

Dengan runtime per thread, setiap socket terdaftar di epoll runtime yang membukanya. Pool koneksi
upstream Pingora dipakai bersama semua thread, jadi koneksi yang dibuka thread B bisa dipakai
request di thread A. Setiap event readiness koneksi itu lalu membangunkan B, dan B membangunkan A
lewat eventfd: rata-rata 0,97 `write()` eventfd per request (dihitung dengan LD_PRELOAD penghitung
syscall). Sejak 2026-11-08 conduit mengisi `group_key` peer dengan nomor worker thread (hanya kalau
`work_stealing: false` dan `threads` > 1). `group_key` ikut dalam kunci pool, jadi setiap thread
memakai koneksinya sendiri. Hasilnya 0,19 eventfd per request, dan A/B terhadap binary sebelumnya:

| Skenario | req/s | p99 | CPU per request |
|---|---:|---:|---:|
| HTTP keep-alive, jenuh | +4,0% | +19,9% (4 dari 7 ronde ±1,3 ms, sisanya ±1,0–1,1 ms) | −4,2% |
| HTTP dengan gzip, jenuh | +2,0% | −3,6% | −1,1% |
| HTTPS keep-alive, jenuh | +2,6% | −0,9% | −2,2% |
| HTTP/2, jenuh | +1,0% | +0,3% | −4,5% |
| HTTP rate tetap 30k req/s | sama | +5,4% (rentang per ronde tumpang tindih) | +2,1% |

Biayanya: koneksi idle ke backend paling banyak `threads` kali lebih banyak, dan request pertama
di setiap thread membuka koneksi sendiri.

Catatan lama: sebelum fork, task Pingora yang mengawasi koneksi idle bisa berjalan di thread lain,
dan kadang (1–5 per 200.000 request) Pingora membuka koneksi baru dengan ERROR "failed to acquire
reusable stream". Patch `79bf391` menghapus task itu beserta pesannya, jadi
`conduit_upstream_reuse_lost_total` sekarang selalu 0.

### Build PGO

Binary release dibangun dengan profile-guided optimization (`scripts/pgo.sh`, dipanggil dari
`Dockerfile`):
1. Build pertama berinstrumen.
2. Binary itu melayani beban latihan `scripts/pgo-train.sh` (±95.000 request, ±5 detik). Bebannya
   mengikuti trafik host produksi: sebagian besar HTTP/1.1 keep-alive dari Cloudflare lewat proxy,
   ditambah HTTPS, HTTP/2, handshake TLS baru, plugin, cache, file server, kompresi, redirect,
   404/403, dan satu reload.
3. Build kedua memakai profil itu, sehingga LLVM menata kode dan memilih fungsi yang di-inline
   berdasarkan jalur yang benar-benar sering dipakai, bukan tebakan.

A/B `scripts/ab.sh` (7 ronde bergantian, median, `work_stealing: false`, proxy di 2 core fisik).
Angkanya dibandingkan dengan binary sebelum optimasi kode terakhir. "Kode" = optimasi di conduit
(pool satu backend tanpa alokasi, request ID tanpa `format!`, header `X-Forwarded-For` dan
`X-Real-IP` berbagi satu nilai), "PGO" = kode + PGO:

| Skenario | req/s kode | req/s PGO | p99 PGO | CPU per request PGO |
|---|---:|---:|---:|---:|
| HTTP keep-alive, jenuh | +0,9% | **+16,3%** (77.959 → 90.685) | −13,0% | −16,1% |
| HTTPS keep-alive, jenuh | +0,5% | **+14,5%** | −13,7% | −14,7% |
| HTTP/2, jenuh | −0,2% | **+13,7%** | −13,5% | −13,3% |
| TLS handshake baru | +1,2% | +5,8% | −1,9% | −3,9% |
| HTTP rate tetap 30k req/s | sama | sama | **−24,5%** (1,10 → 0,83 ms) | −15,8% |

PGO menang di setiap ronde: di HTTP, HTTPS dan HTTP/2, ronde PGO paling lambat masih lebih cepat
dari ronde baseline paling cepat. Optimasi kode sendiri masih dalam batas noise, karena hot path
conduit memang tinggal ±13% CPU. Handshake naik paling sedikit, karena sebagian besar waktunya
ada di OpenSSL (C), yang tidak ikut diinstrumentasi.

Yang sengaja tidak dipakai:
- `panic = "abort"`: satu panic di satu request akan mematikan seluruh proxy. Dengan unwind,
  Pingora menjalankan setiap koneksi sebagai task Tokio sendiri, dan Tokio menangkap panic di
  task itu: hanya koneksi tersebut yang putus.
- Runtime thread-per-core lain (monoio, glommio): berarti keluar dari Pingora. Runtime per thread
  di dalam Pingora sudah tersedia lewat `work_stealing: false`.
- Arena per request (bumpalo): alokasi yang tersisa ada di dalam Pingora dan `http`, bukan di
  kode conduit.

`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip` dan mimalloc sudah aktif sejak awal.
Tabel benchmark di atas diambil dari release PGO (`dist/conduit`).

### Profil CPU

Diukur dengan samply (4 kHz, 150.000 request HTTP/1.1, access log aktif, build biasa). Pembagian
CPU worker:

| Bagian | Pingora asli | Setelah putaran ketiga patch |
|---|---:|---:|
| Syscall (terutama `send`, termasuk stack TCP kernel) | 36% | 44% |
| Pingora + tokio di user space | ±51% | ±46% |
| Kode conduit (hook, plugin, access log) | ±13% | ±10% |

Porsi syscall naik karena user space menyusut; jumlah syscall per request tetap.

Dengan Pingora asli, bagian Pingora tersebar di alokasi (mimalloc ±10% dari semua sampel),
`HeaderMap` ganda untuk menjaga kapitalisasi nama header, clone header, dua channel per request,
dan memcpy future async. Kode conduit terbesar adalah `upstream_request_filter` (5%;
`insert_header` Pingora mem-parse nama header setiap kali) dan access log (3,5–4%).

Setelah putaran ketiga, `HeaderMap` ganda dan clone header respons tidak ada lagi, channel turun
dari 3,1% ke 1,5%, `upstream_request_filter` 3,3%, dan `logging` 1,4% (histogram dan baris access
log; menulis ke file dikerjakan thread lain). Tidak ada lagi satu titik di user space yang lebih
dari ±1,5%: sisanya alokasi, memcpy future, dan kode perekat proxy Pingora.

Temuan dari profil ini yang sudah diperbaiki: buffer tulis L4 bawaan Pingora hanya 1460 byte,
sehingga header dan body respons 2 KB terkirim dalam dua `send()` terpisah dan klien membalas
ACK di antaranya: 6 segmen TCP per request, sedangkan nginx 4. Dengan `server.write_buffer: 16k`
conduit kini juga 4 segmen per request, throughput HTTP naik 11% dan sys time turun 18%.

Profil ulang 2026-11-08 menunjukkan pool koneksi upstream Pingora memakan ±5,4% sampel (spawn
task dan timer per request saat koneksi dikembalikan, `getpeername()` saat dipakai ulang) dan
scheduler tokio ±6,4%. Setelah [patch Pingora](../vendor/README.md), angkanya ±3,6% (sebagian besar
membebaskan buffer respons, yang memang harus terjadi) dan ±4,0%. Bagian lain tetap seperti tabel
di atas.

### Catatan memori

Dengan kebijakan kernel `transparent_hugepage=always` (termasuk di host produksi), arena
mimalloc terisi huge page 2 MB, dan idle RSS hampir dua kali lipat. conduit mematikan THP untuk
prosesnya sendiri (`server.transparent_huge_pages: false`). Mengaktifkannya memberi throughput
±1% lebih tinggi. Health check semua pool berbagi satu set connector, karena setiap connector
OpenSSL memuat seluruh CA store sistem. Buffer tulis 16 KiB per koneksi hanya terisi sebesar
respons yang lewat.

## Perintah

```sh
mise run test                 # unit + e2e
mise run test-acme            # ACME end-to-end dengan Pebble (butuh Docker)
mise run test-crowdsec        # CrowdSec end-to-end dengan CrowdSec asli (butuh Docker)
mise run lint                 # fmt + clippy
mise run release              # dist/conduit (PGO) untuk Ubuntu 22.04 (glibc ≤ 2.35), via Docker
mise run pgo                  # build PGO lokal ke target/pgo/conduit (butuh rustup llvm-tools)
mise run bench                # conduit vs nginx (gaya NPM dan tuned), butuh Docker
mise run compare              # dist/conduit vs nginx, Caddy, Apache, HAProxy, Traefik, Envoy (Docker)
mise run ab base=BIN new=BIN  # A/B binary atau setting (mis. new=BIN,work_stealing=true), butuh Docker
scripts/soak/deploy.sh HOST [JAM]  # soak test dist/conduit di mesin uji sekali pakai (lihat di bawah)
fuzz/run.sh [DETIK]           # semua target fuzz paralel, default 300 detik (butuh nightly + cargo-fuzz)

conduit check -c conduit.yaml
conduit run -c conduit.yaml [-d] [-u]
conduit import-npm --npm-root ./salinan-npm --paths-root /opt/npm > conduit.yaml
scripts/parity.py CONFIG IP_NPM IP_CONDUIT --http-port 80,18080 --https-port 443,18443
scripts/parity/run.sh SALINAN_NPM SERTIFIKAT_UJI [OVERLAY] [KASUS]  # paritas lokal, butuh Docker
```

Fuzzing (`fuzz/`) memakai cargo-fuzz. Target-nya ada di `src/fuzzing.rs`, karena itu conduit
dibangun sebagai library plus binary tipis, supaya target fuzz bisa memanggil fungsi internal.
Seed ada di `fuzz/seeds/`, sedangkan corpus yang tumbuh (`fuzz/corpus/`) dan crash
(`fuzz/artifacts/`) tidak di-commit. Setiap target juga mengecek sifat yang dijanjikan kode,
tidak hanya bebas panic:

| Target | Sifat yang dicek |
|---|---|
| `config` | parser YAML; config yang diterima lalu dikompilasi (hanya backend IP, tanpa DNS) |
| `import` | output `import-npm` harus bisa dimuat sebagai config |
| `path` | hasil normalisasi selalu path absolut tanpa segmen kosong, `.`, atau `..` |
| `access_log` | JSON tulisan tangan sama persis dengan serde dan tetap satu baris |
| `request` | host huruf kecil, real IP hanya dari peer tepercaya, status jawaban 100–599, target tanpa path → 400, range dalam batas file |
| `route` | target hasil aksi `proxy` (`strip_prefix`, `add_prefix`, `rewrite`) selalu diawali `/`, hanya berisi karakter path atau escape `%XX`, query tidak berubah, dan diterima Pingora |
| `body_replace` | hasil penggantian sama persis, bagaimanapun body dipotong menjadi chunk (1–7 byte, berselang-seling) |
| `block_exploits` | input apa pun sebagai query, path, user agent dan header: tanpa panic, dan kata kunci yang memilih rule tidak pernah menyembunyikan kecocokan (hasil dengan dan tanpa kata kunci selalu sama) |

Soak test (`scripts/soak/`) berjalan di mesin uji sekali pakai sebagai unit systemd sementara
`conduit-soak`. Skrip ini memasang nginx dan menambah tabel nftables di mesin itu, dan semua
layanannya hanya mendengar di 127.0.0.1. Isinya:

- **Backend:** nginx di dua port dan server echo WebSocket.
- **Beban:** ±650 req/s lewat oha dalam jendela 5 menit (HTTP/1.1 GET 256 KiB gzip dan POST,
  HTTPS, HTTP/2, handshake TLS baru per request), ditambah 40 WebSocket yang hidup lama dan
  satu WebSocket pendek tiap 0,5 detik.
- **Event berkala:**

  | Event | Interval |
  |---|---|
  | Reload | 5 menit |
  | Satu backend ditolak 30 detik | 10 menit |
  | Access log dirotasi | 30 menit |
  | Sertifikat dirotasi | 1 jam |
  | Graceful upgrade (`run -d -u` lalu SIGQUIT, seperti `systemctl reload`) | 4 jam |

- **Sampel tiap 30 detik:** RSS, fd, thread, CPU, koneksi, dan metrik.

Hasilnya ada di `~/soak/out/` di mesin itu, dengan ringkasan di `report.txt`. Untuk uji asap
harness, `SOAK_SCALE=40 SOAK_SECONDS=420` mempercepat semua interval.

Contoh config lengkap dengan semua opsi: [`conduit.example.yaml`](../conduit.example.yaml).

Config ditulis dalam YAML 1.2, dibaca dengan [serde-saphyr](https://github.com/bourumir-wyngs/serde-saphyr)
(pure Rust, tanpa panic, dengan batas parsing untuk anchor dan alias):

- hanya `true`/`false` yang boolean; `yes`, `no`, `on` ditolak di field boolean, dan `off`
  tetap kata biasa (`upgrade: off`)
- field yang tidak dikenal dan key ganda ditolak
- bagian yang isinya dikomentari semua (`default_site:` saja) sama dengan tidak ditulis
- anchor (`&base`) dan merge key (`<<: *base`) bisa dipakai untuk site yang mirip
- error menyebut baris dan kolom, tanpa mengutip isi file (file berisi secret, dan error
  reload masuk log dan `/status`)
Unit systemd dan logrotate: [`deploy/`](../deploy/). Parser dan acquisition CrowdSec:
[`deploy/crowdsec/`](../deploy/crowdsec/).

## Rencana

- **Fase 1 (selesai 2026-11-08):** target throughput HTTP/1.1 dan HTTPS diubah menjadi
  ≥0,75× nginx tuned atau CPU per request ≤1,4×. Sepuluh patch Pingora (tiga putaran dan tahap
  A) ditambah access log baru membawa ±0,6× ke 0,76–0,81×; lihat
  [Throughput HTTP/1.1 dan HTTPS](#throughput-http11-dan-https). Patch yang terbukti bisa
  diajukan ke upstream Pingora ([`vendor/README.md`](../vendor/README.md)).
- **Fase 2 (selesai):** ACME bawaan, CrowdSec bouncer + AppSec, rate limit dan batas koneksi,
  mTLS, `basic_auth` dan `forward_auth`. Ditambah: file server statis dan cache respons.
- **Fase 3 (berikutnya): stabilitas inti, tanpa fitur baru.** Inti = routing, TLS,
  WebSocket/upgrade, reload dan graceful upgrade, upstream pool dan health check, access log.
  Urutan:
  1. ✓ Uji upstream flapping di bawah beban (selesai 2026-11-08, `upstream_flapping` di
     [Hasil uji](#hasil-uji-lokal)). Temuan: timeout upstream dijawab 502, sekarang 504.
  2. Soak test beban campuran (HTTP/1.1, HTTPS, HTTP/2, WebSocket, reload berkala), target 24
     jam. RSS, fd, koneksi upstream, dan jumlah task tidak naik terus. Harness:
     `scripts/soak/` (lihat [Perintah](#perintah)). Run 24 jam berjalan di mesin uji sejak
     2026-11-08 16:32 UTC.
  3. ✓ Fuzzing (`fuzz/`, selesai 2026-11-09): parser config, `import-npm`, normalisasi path,
     access log, dan request (host, real IP, routing, plugin, rule, hash LB, parser header).
     Temuan: `import-npm` menulis config yang tidak bisa dimuat kalau NPM tidak punya sertifikat
     sama sekali, atau kalau file NPM berisi karakter kontrol (CR di advanced config bahkan
     bisa mengubah komentar jadi YAML aktif); target tanpa path (`GET ?x`) sampai ke plugin
     sebagai `/`, sekarang 400.
  4. ✓ Audit panic dan klien bermasalah (selesai 2026-11-09). 33 `unwrap`/`expect` di kode
     non-test, tidak ada yang bergantung pada input (mutex poison, invarian, startup).
     Temuan: koneksi keep-alive idle tidak pernah ditutup (sekarang `keepalive_timeout`), dan
     slowloris bisa menahan koneksi selamanya (patch Pingora `7ffe1c4`). fd habis dan header
     besar sudah aman.
  5. ✓ Graceful upgrade dengan WebSocket terbuka (selesai 2026-11-09). Temuan: koneksi yang
     diputus saat proses lama berhenti hilang dari access log; sekarang tercatat.
  6. ✓ Uji paritas lokal (selesai 2026-11-09, lihat [Hasil uji](#hasil-uji-lokal)). Salinan
     config NPM di laptop identik dengan host produksi (checksum). Beda perilaku yang tersisa
     hanya pada "Block Exploits" (aktif di 38 dari 51 host), yang kini diimpor sebagai plugin
     `block_exploits`: conduit tidak lagi salah blokir seperti NPM, dan menangkap serangan
     yang lolos dari NPM.
- **Ditunda sampai inti stabil:** kunci session ticket bersama antar proses, OCSP, admin API,
  OpenTelemetry, captcha CrowdSec, DNS-01. CrowdSec dan AppSec tidak dikembangkan lagi; yang ada
  tetap, dan opsional saat migrasi. Paket `.deb`/image container dan dokumentasi dikerjakan
  menjelang migrasi.
- **Opsional:** jalur cepat HTTP/1→HTTP/1 di fork Pingora, kalau kelak setara nginx dibutuhkan.
