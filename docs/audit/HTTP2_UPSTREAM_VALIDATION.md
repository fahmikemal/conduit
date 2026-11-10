# HTTP/2 upstream: implementasi dan validasi

Tanggal: 10 November 2026. Perubahan ini menambahkan HTTP/2 upstream untuk layanan internal tanpa mengubah default HTTP/1.1 pada konfigurasi lama. Pengujian menggunakan binary Conduit nyata dan server HTTP/2 dengan framing/flow control, bukan mock respons HTTP/1 yang diberi label HTTP/2.

Implementasi yang diuji sudah masuk commit `03cae2b`. [Indeks audit](README.md) merangkum
penutupan temuan dan [referensi konfigurasi](../configuration.md#4-upstream-pools) menjadi
rujukan opsi. Perubahan Markdown berikutnya tidak mengganti binary atau source yang diuji.

## Pemakaian

Untuk layanan internal HTTP/2 tanpa TLS, konfigurasi minimal yang dapat di-check:

```yaml
listen:
  http: ["127.0.0.1:8080"]
sites:
  - id: internal
    hosts: [internal.example.com]
    upstream: h2c://127.0.0.1:50051
```

Untuk TLS, HTTP/2 wajib atau negosiasi otomatis:

```yaml
upstreams:
  grpc:
    backends: [https://grpc.internal:443]
    http_version: "2"
    http2_max_streams: 128
  web:
    backends: [https://web.internal:443]
    http_version: auto
```

| Pilihan | HTTP tanpa TLS | HTTPS |
|---|---|---|
| Default / `"1.1"` | HTTP/1.1 | HTTP/1.1 |
| `"2"` | HTTP/2 prior knowledge | HTTP/2 wajib; koneksi tanpa negosiasi H2 ditolak sebelum request aplikasi dikirim |
| `auto` | HTTP/1.1 | ALPN H2/H1; fallback HTTP/1.1 diperbolehkan |
| URL `h2c://` | HTTP/2 wajib | Tidak memakai TLS |

`h2c://` tidak boleh digabung dengan `http_version: "1.1"` atau `auto`. Nilai `http_version: 2` sebagai angka juga diterima. `http2_max_streams` harus 1–4294967295 dan dibatasi lagi oleh SETTINGS server. Native gRPC pada sisi klien memakai HTTP/2 melalui listener TLS Conduit; fitur ini tidak menambahkan h2c pada listener klien atau extended CONNECT/RFC 8441.

Default reverse proxy memang bervariasi: NGINX beralih dari HTTP/1.0 ke HTTP/1.1 pada 1.29.7, sedangkan Caddy mengizinkan HTTP/1.1 dan HTTP/2 dalam transport default. Referensi: [NGINX](https://blog.nginx.org/blog/keep-alive-to-upstreams-is-now-default-in-nginx-1-29-7), [Caddy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy). Default Conduit sengaja tetap kompatibel dengan konfigurasi sebelumnya.

## Perubahan implementasi

| File | Perubahan |
|---|---|
| [src/config.rs](../../src/config.rs) | `UpstreamHttpVersion`, `http_version`, validasi bentuk input, dan `http2_max_streams`. |
| [src/upstream.rs](../../src/upstream.rs) | Resolusi `h2c://`, preferensi peer, signature pool, pemisahan identitas protokol, health check H2, canonical Host IPv6, dan pelepasan koneksi probe. |
| [src/proxy.rs](../../src/proxy.rs) | Pilihan protokol, enforcement H2 sebelum request dikirim, pemisahan koneksi H1/H2/upgrade, dan pelestarian worker group H1 lama. |
| [src/admin.rs](../../src/admin.rs) | `/status` melaporkan policy HTTP tiap backend; `auto` merupakan policy, bukan hasil negosiasi terakhir. |
| [tests/support/h2_upstream.rs](../../tests/support/h2_upstream.rs) | Server H1/H2 nyata, lima tes fungsional, serta soak lokal yang dijalankan eksplisit. |
| [vendor/pingora-h2-reuse.patch](../../vendor/pingora-h2-reuse.patch) | Backport race pool HTTP/2 beserta tes deterministik upstream. |
| [scripts/prepare-vendor.sh](../../scripts/prepare-vendor.sh) | Penerapan patch idempotent: source bersih, sudah terpatch, dan source tidak cocok diuji. |
| Dockerfile, mise.toml, scripts/pgo.sh | Jalur build/test yang didokumentasikan menyiapkan backport otomatis. Native `cargo` langsung memerlukan persiapan vendor terlebih dahulu. |

HTTP/1 upgrade seperti WebSocket tetap memakai H1 ketika policy `auto`; policy H2 wajib menolak forwarded upgrade dengan 400. HTTP health check mengikuti protokol backend; TCP check memeriksa transport saja. Probe memakai satu stream per koneksi sehingga pembatalan/timeout tidak meninggalkan entry koneksi di pool H2. Koneksi probe dilepas dengan idle timeout nol.

Peer reuse hash Pingora tidak mencakup ALPN. Karena itu group key memasukkan domain protokol: dua bit tertinggi ketika memakai pool per worker; domain H1 bernilai nol sehingga identitas worker H1 lama tetap sama. Batas stream sudah masuk reuse hash Pingora, dan perubahan protokol/batas stream masuk signature pool agar reload tidak mempertahankan konfigurasi lama.

## Perbaikan dependency yang diperlukan

Fork tetap pada gitlink `5c765c66590bdb7829ad32e00abc6f9a4e0410af`, ditambah patch yang disimpan di repository ini. Tidak ada upgrade seluruh dependency/fork.

[Issue Pingora #1024](https://github.com/cloudflare/pingora/issues/1024) menjelaskan race `reused_http_session` dengan `release_http_session`: entry `InUsePool` dapat terduplikasi dan menahan socket setelah idle timeout. Backport [PR #1025](https://github.com/cloudflare/pingora/pull/1025), head `afe1c1031ead32642cddf920bbf2be0fd2bb8703`, mengunci `release_lock` dan menghapus entry lama sebelum insert.

Tes native `test_reused_http_session_no_duplicate_in_use_entry` pada source fork terpatch: **1 passed**, 634 unit lain tidak dipilih. Tes memaksa interleaving melalui test peer dan memeriksa tidak ada entry tertinggal setelah idle timeout. [Log native](http2-pool-race.log). Workspace test vendor memakai lock lokalnya sendiri; build aplikasi tetap memakai Cargo.lock root dengan `--locked`.

```sh
bash scripts/prepare-vendor.sh
cargo test --manifest-path vendor/pingora/Cargo.toml -p pingora-core \
  --features openssl test_reused_http_session_no_duplicate_in_use_entry -- --nocapture
```

Source submodule terlihat modified setelah persiapan, sesuai penerapan patch lokal. Docker juga dapat menyiapkan source submodule bersih karena patch dan script ikut disalin ke build context.

## Uji fungsional dan regresi

| Tes | Bukti utama |
|---|---|
| `upstream_http2_cleartext_reload_and_multiplexing` | URL h2c langsung, HTTP/1 → HTTP/2 → HTTP/1 pada pool yang sama, serta stream yang benar-benar overlap pada satu koneksi H2. |
| `upstream_http2_tls_negotiation_strict_and_upgrade` | H1 default, ALPN H2, fallback H1, H2 wajib menolak server tanpa ALPN tanpa mengirim request H1; upgrade echo melewati H1 pada policy auto. |
| `upstream_http2_health_cache_and_validation` | HTTP health probe H2 dengan authority IPv6, miss/hit cache dan authority/scheme benar; konfigurasi invalid ditolak tanpa panic. |
| `upstream_http2_grpc_bidirectional_stream_and_trailers` | Respons tiba sebelum request EOF, frame gRPC 2 MiB melampaui window klien awal, reload saat stream terbuka, trailer sukses/status 7, cancellation dan request berikutnya pada koneksi klien yang sama. |
| `upstream_http2_stream_limit_and_reload` | Batas satu stream per koneksi benar-benar berlaku, lalu reload ke empat memungkinkan overlap 2–4 stream. |

Lima tes diulang pada release build final. [Log debug](http2-e2e.log), [log release final](http2-final-release-e2e.log).

Suite lengkap aplikasi: **109 unit biasa + 2 unit pengukuran, 30 e2e biasa + ACME + CrowdSec + slowloris + soak H2 = 111 unit dan 34 e2e lulus**. Empat e2e yang ignored secara default dijalankan eksplisit. Clippy dengan `-D warnings` dan rustfmt lulus. [Suite](http2-full-tests.log), [Clippy](http2-clippy.log), [ACME](http2-acme.log), [CrowdSec](http2-crowdsec.log), [slowloris](http2-slowloris.log), [unit pengukuran](http2-ignored-unit.log).

Percobaan ACME pertama gagal ketika container Pebble tidak tersedia pada operasi copy fixture Podman; pengulangan terisolasi lulus, termasuk renewal saat traffic. Ini kegagalan startup fixture yang teramati, bukan bukti kegagalan aplikasi atau diagnosis pasti penyebab container berhenti.

PoC temuan audit lama diulang pada binary release final; seluruh assertion closure dan kontrol perilaku lulus. [Hasil](http2-final-closure-results.json).

## Performa

Benchmark awal menemukan penurunan default H1 sekitar 12–13%. Implementasi kemudian mempertahankan identitas worker H1 lama, dan pengukuran diulang dengan sampel lebih besar. Kedua hasil disimpan; hasil awal bukan hasil final. [Log awal](http2-default-performance-initial.log), [data awal](http2-default-performance-initial-raw.jsonl).

Perbandingan final: build release sebelum fitur dan build final, compiler/default features/LTO/lock aplikasi yang sama; lima putaran bergantian, 200.000 request per skenario per varian, concurrency 64, body 2 KiB, access log aktif. Total **6.000.000 request terukur, nol error**, tidak ada access log yang hilang.

| Skenario | Sebelum req/s | Sesudah req/s | Perubahan | P99 sebelum → sesudah |
|---|---:|---:|---:|---:|
| HTTP/1 | 75.706 | 76.858 | +1,5% | 1,55 → 1,47 ms |
| HTTPS | 55.412 | 60.584 | +9,3% | 2,12 → 1,77 ms |
| HTTP/2 sisi klien | 47.267 | 48.456 | +2,5% | 2,57 → 2,48 ms |

Ketiga skenario ini memakai **upstream HTTP/1.1 default** dan memeriksa kompatibilitas/performa jalur lama. Hasil tidak mengukur perbandingan peak H1 versus H2 upstream. H2 upstream diukur pada offered-load tetap melalui soak di bawah. [Log final](http2-default-performance.log), [30 sampel final](http2-default-performance-raw.jsonl).

Host dipakai bersama dan scheduling/frekuensi CPU berubah antar-run. Proxy dipin CPU 0,2 dan generator 6–11; rootless nginx tetap memiliki allowed CPU 0–11 meskipun wrapper mencoba membatasi upstream. Karena itu tidak ada klaim peningkatan universal atau atribusi kausal pasti dari angka median.

## Stabilitas H2 upstream

Soak berjalan pada HTTP/2 upstream melalui TLS, offered-load 1.500 req/s selama 120 detik, 32 koneksi klien, health probe aktif, reload dan pergantian batas stream tiap lima detik. Sesudah traffic berhenti, tiga detik diberikan untuk idle timeout satu detik; FD akhir dibandingkan baseline sebelum beban.

Hasil final disimpan pada [http2-release-stability-results.json](http2-release-stability-results.json) beserta seluruh sample RSS/FD dan hasil generator. Run debug dan release awal dipertahankan sebagai pembanding. [Log release](http2-release-stability.log), [hasil debug](http2-stability-results.json).

Run release final: **179.999 request, nol error, 23 SIGHUP**. FD sebelum beban 40 dan setelah idle 39; RSS setelah warmup 28.152 → 29.596 KiB (bertambah sekitar 1,41 MiB). Seluruh request yang terukur berstatus 200. Kenaikan resource tetap terbatas pada durasi/workload ini; tidak ada klaim pembuktian bebas leak untuk semua kondisi.

```sh
CONDUIT_HTTP2_TEST_BIN=/path/to/release/conduit \
OHA_BIN=/path/to/oha CONDUIT_H2_REPORT=/path/to/results.json \
cargo test --locked --test e2e upstream_http2_load_stability -- --ignored --nocapture
```

Soak ini merupakan pengujian lokal terbatas. Run 24 jam Krynox yang disebut pengguna tidak diubah maupun diklaim telah diverifikasi oleh pengujian ini.

## Integritas dan pemetaan

Hash source, binary, patch dan hasil pemeriksaan final berada di [http2-validation.json](http2-validation.json).
Graphify disinkronkan setelah implementasi, dengan kode, dokumentasi policy HTTP, tes H2
dan backport rationale. [Bukti sinkronisasi H2](http2-graphify-resync.json) merekam snapshot
graph pada putaran tersebut; hash source/dokumen di sana tidak dimaksudkan berubah mengikuti
edit Markdown berikutnya. [documentation-sync.json](documentation-sync.json) mencatat
pemeriksaan dan graph terbaru untuk putaran dokumentasi ini.

Context7 digunakan untuk kontrak PeerOptions/ALPN Pingora dan framing/flow control/trailer crate h2. Source fork terkunci dan pengujian wire menentukan perilaku aktual. Menambahkan H2 tidak berarti seluruh API Pingora sudah diekspos sebagai opsi konfigurasi Conduit.
