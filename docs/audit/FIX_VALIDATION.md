# Perbaikan dan validasi penutupan temuan

Tanggal: 10 November 2026. Laporan ini mencatat putaran perbaikan audit **sebelum fitur
HTTP/2 upstream**. Bukti dan angka 111 unit / 28 E2E di bawah berlaku pada putaran itu.
Implementasi final `03cae2b` mengulang seluruh suite dan closure: **111 unit / 34 E2E lulus**,
seperti dicatat di [HTTP2_UPSTREAM_VALIDATION.md](HTTP2_UPSTREAM_VALIDATION.md). [Indeks audit](README.md)
merangkum status sekarang; bukti sebelum perbaikan dipertahankan sebagai riwayat.

Perbaikan implementasi F01/F02/F03/F05/F06/F07/F09, perbaikan warning serta penegasan override wildcard F08, dan koreksi dokumentasi F10 sudah diterapkan. F04 tetap ditarik sebagai bug karena sesuai kontrak cache yang didokumentasikan. Delapan tes e2e regresi baru memakai binary nyata; masing-masing dijalankan setelah perbaikan terkait.

## Matriks penutupan

| ID | Perubahan | Bukti setelah perbaikan |
|---|---|---|
| F01 | Pisahkan normalisasi raw path dari normalisasi path yang sudah di-decode. Candidate file tidak menafsirkan `%` lagi. | `security_file_path_decoded_once`: ordinary/single-encoded protected path 403; double-encoded 404; nama file literal `%61dmin` tetap dapat dibaca dengan isi yang benar. |
| F02 | Ganti canonicalize-then-open dengan traversal descriptor `openat(O_NOFOLLOW)`. Listing memakai directory descriptor yang sudah diverifikasi dan mengabaikan symlink. | `security_file_fallback_containment`: seccomp pada child memaksa `openat2` mengembalikan ENOSYS; kontrol file normal berhasil, symlink keluar ditolak, 600 HTTP lookup/listing selama replacement tidak mengungkap marker luar. PoC Rust fungsi asli juga mengulang 100.000 percobaan tanpa pembacaan luar. |
| F03 | Authority asli, termasuk port, masuk identitas cache sebagai komponen dengan batas panjang. | `security_cache_authority`: origin membedakan Host/XFH 8080 dan 9090; kedua varian berisi authority yang benar dan tetap memperoleh cache hit masing-masing. |
| F04 | Tidak memaksakan invalidasi fresh cache saat upstream berubah. | Skrip closure tetap membuktikan A bertahan setelah reload upstream B, kemudian purge menghasilkan B. Ini perilaku terdokumentasi. |
| F05 | Perkalian menit/jam/hari memakai `checked_mul` dan mengembalikan error konfigurasi. | `security_duration_overflow_rejected`: tiga input overflow ditolak CLI check dan reload; layanan lama tetap 200, lalu reload konfigurasi valid berhasil, tanpa panic. |
| F06 | Validasi watch interval positif serta dapat direpresentasikan oleh runtime clock sebelum runtime diterbitkan. | `security_zero_watch_interval_rejected`: 0s ditolak check/reload; layanan aktif tetap hidup dan reload valid pulih. |
| F07 | Kunci admission pada 64 shard mengikat pembacaan allowance dan pencatatan pemakaian untuk key yang sama. | `security_rate_limit_concurrent_admission`: 12 putaran × 32 request serentak pada 8 worker; setiap putaran tepat 1 diterima dan 31 ditolak. Reload ditunggu melalui kenaikan counter sukses; allowance yang sudah terpakai tetap ditolak sesudahnya. |
| F08 | Pisahkan wildcard override dari exact override; perbaiki coverage lookup untuk pola wildcard. | `security_tls_wildcard_override_and_coverage`: konfigurasi wildcard SAN tidak lagi mendapat warning palsu; handshake nyata memilih wildcard override yang diminta, exact override tetap menang, wildcard tidak mencakup dua label. |
| F09 | Untuk Vary eksplisit, variance mempertahankan seluruh nilai header dan batas setiap field, termasuk coding tidak dikenal, bobot, serta absent/empty. | `security_cache_unknown_encoding`: deflate dan identity tidak berbagi representasi; kedua varian memperoleh hit sendiri; payload deflate dari cache benar-benar di-decode dan cocok dengan marker. Existing unit variance ikut lulus. |
| F10 | README menjelaskan bahwa `server_header: off` menghentikan penambahan header oleh conduit/Pingora, sementara header origin diteruskan. | Existing e2e `server_header` lulus, termasuk origin passthrough dan penghapusan melalui plugin. |

Semua tes regresi di atas berada di [tests/e2e.rs](../../tests/e2e.rs). Hasil individual disimpan sebagai `fix-f01.log` sampai `fix-f10.log` (F04 memakai closure perilaku, bukan perubahan aplikasi).

Peta file patch:

| File | Bagian yang berubah |
|---|---|
| [src/pathnorm.rs](../../src/pathnorm.rs) | `normalize_decoded` memisahkan penyusunan segmen dari percent-decoding raw input. |
| [src/files.rs](../../src/files.rs) | `Files::find`, fallback `open_beneath`, dan `listing` memakai representasi/path descriptor yang konsisten. |
| [src/proxy.rs](../../src/proxy.rs) | `cache_key_callback` memasukkan authority request. |
| [src/cache.rs](../../src/cache.rs) | `variance` memakai framing nilai header, beserta koreksi ekspektasi unit untuk ejaan encoding ekuivalen. |
| [src/units.rs](../../src/units.rs) | `parse_duration` menolak perkalian yang overflow. |
| [src/runtime.rs](../../src/runtime.rs) | `build` memvalidasi watch interval sebelum menyiapkan runtime. |
| [src/limits.rs](../../src/limits.rs) | `Window` dan admission `RateLimit::request` mengunci keputusan serta observe. |
| [src/tls.rs](../../src/tls.rs) | `CertStore` menyimpan wildcard override terpisah dan membetulkan `select_specific`. |
| [README.md](../../README.md), [docs/configuration.md](../configuration.md) | Selaraskan kontrak header, fallback files, decoding, admission dan prioritas sertifikat. |
| [tests/e2e.rs](../../tests/e2e.rs) | Delapan tes regresi baru beserta helper origin, invalid reload, dan pemaksaan syscall fallback. |

## Hasil suite dan pemeriksaan

| Pemeriksaan | Hasil | Bukti |
|---|---|---|
| `cargo test --locked --all-targets` | 109 unit lulus; 25 e2e lulus; 2 unit pengukuran dan 3 e2e khusus diabaikan oleh default suite | [Log suite](fix-full-tests.log) |
| Unit yang diabaikan, dijalankan eksplisit | 2/2 lulus | [Log](fix-ignored-unit.log) |
| E2e ACME dengan Pebble | 1/1 lulus; penerbitan, renewal saat traffic, serta policy refusal | [Log](fix-acme.log) |
| E2e CrowdSec dengan LAPI/AppSec nyata | 1/1 lulus; ban, prober, body inspection dan parser | [Log](fix-crowdsec.log) |
| E2e slowloris | 1/1 lulus | [Log](fix-slowloris.log) |
| `cargo clippy --locked --all-targets -- -D warnings` | Lulus | [Log](fix-clippy.log) |
| `cargo fmt --all --check` | Lulus | [Log kosong berarti sukses](fix-fmt.log) |
| Recheck HTTP/TLS dengan input PoC semula | Semua assertion closure dan kontrol perilaku lulus | [JSON](fix-closure-results.json), [script](recheck_runtime.py) |
| Recheck race fallback Rust | 100.000 percobaan, tidak ada marker luar; kontrol file biasa dan penolakan symlink lulus | [Log](fix-fallback-closure.log) |
| Pengulangan regresi setelah soak | Delapan tes `security_` lulus kembali | [Log terakhir](fix-final-security.log) |
| Penutupan terakhir pada binary release | Semua assertion PoC dan kontrol lulus kembali pada build teroptimisasi | [JSON terakhir](fix-final-closure-results.json), [log](fix-final-closure.log) |

Total: **111 unit dan seluruh 28 e2e lulus**; tidak ada e2e yang tersisa hanya sebagai ignored. Dua unit berlabel ignored merupakan pengukuran dev-profile dan tidak dipakai sebagai bukti performa release.

Perintah pokok:

```sh
bash scripts/prepare-vendor.sh
cargo test --locked --all-targets
cargo test --locked --lib -- --ignored --nocapture
cargo test --locked --test e2e acme_with_pebble -- --ignored --nocapture
cargo test --locked --test e2e crowdsec_bouncer_and_appsec -- --ignored --nocapture
cargo test --locked --test e2e slow_request_head_is_cut_off -- --ignored --nocapture
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all --check
python3 docs/audit/recheck_runtime.py /path/to/fixed/conduit \
  --expect-fixed --out docs/audit/fix-closure-results.json
cargo test --locked --test e2e security_ -- --nocapture
```

Nama filter pada tiga perintah ignored mengacu pada fungsi tes terkait di suite; log merekam tes yang benar-benar dipilih. Lingkungan ini memakai Podman rootless melalui wrapper kompatibilitas Docker untuk tes yang memerlukan container. Wrapper menambahkan label-disable untuk bind mount dan nama registry eksplisit; source tes tidak diubah untuk melonggarkan assertion.

## Performa release sebelum/sesudah

Build pembanding menggunakan source sebelum perbaikan dan build hasil perbaikan, Cargo.lock yang sama, default features, vendored OpenSSL, compiler, optimisasi release dan fat LTO yang sama. Lima putaran bergantian urutan varian, concurrency 64, payload 2 KiB, access log aktif. Proxy dibatasi CPU 0,2; generator CPU 6–11. Nginx berjalan sebagai container rootless.

**Batas affinity:** rootless cgroup tidak mendelegasikan cpuset. Wrapper menerjemahkan opsi Docker ke `taskset`, tetapi pemeriksaan `/proc` menunjukkan nginx tetap memiliki allowed CPU 0–11. Karena itu hasil ini tidak diklaim memiliki upstream yang dipin CPU 4,5. Kondisi upstream sama bagi kedua varian; variasi penjadwalan mesin tetap memengaruhi angka.

Median benchmark utama, 50.000 request per skenario per varian per putaran (handshake 10.000; offered-load 125.000 pada 10.000 req/s):

| Skenario | Sebelum req/s | Sesudah req/s | Perubahan throughput | P99 sebelum → sesudah |
|---|---:|---:|---:|---:|
| HTTP/1 | 81.017 | 76.491 | −5,6% | 1,60 → 1,60 ms |
| HTTPS | 52.070 | 51.655 | −0,8% | 2,10 → 2,67 ms |
| HTTP/2 | 42.754 | 42.974 | +0,5% | 2,68 → 2,74 ms |
| TLS handshake baru | 4.500 | 4.305 | −4,3% | 27,21 → 31,55 ms |
| Gzip | 55.836 | 53.823 | −3,6% | 2,16 → 2,19 ms |
| Offered-load 10.000/s | 9.998 | 9.998 | sekitar 0% | 0,72 → 0,80 ms |

Seluruh skenario memiliki **0 error**, dan harness memverifikasi tidak ada access log yang hilang/dropped. [Log utama](fix-performance.log), [60 sampel mentah](fix-performance-raw.jsonl).

Karena P99 HTTPS pada pengukuran utama meningkat, HTTPS diulang lima putaran dengan **150.000 request** per varian per putaran: median 56.600 → 60.238 req/s (**+6,4%**), P99 2,13 → 1,88 ms (**−11,7%**), CPU 29,20 → 27,47 ms/1.000 request. [Log pengulangan](fix-tls-performance.log), [sampel](fix-tls-performance-raw.jsonl). Hasil antar-run tidak mendukung kesimpulan perlambatan HTTPS yang konsisten; juga tidak membuktikan speedup universal.

Jalur rate limiter diukur terpisah dengan plugin aktif dan limit tinggi pada key IP yang sama, lima putaran, 100.000 request HTTP per varian serta 250.000 request offered-load 20.000/s:

| Skenario dengan limiter | Sebelum req/s | Sesudah req/s | P99 sebelum → sesudah | CPU ms/1.000 sebelum → sesudah |
|---|---:|---:|---:|---:|
| Saturated HTTP | 76.324 | 74.817 (−2,0%) | 1,46 → 1,45 ms | 21,10 → 21,60 |
| Offered-load 20.000/s | 19.996 | 19.997 | 1,24 → 1,21 ms | 27,56 → 27,48 |

Semua request berhasil, access log lengkap. [Log limiter](fix-rate-performance.log), [sampel](fix-rate-performance-raw.jsonl). Hasil berlaku untuk workload ini; distribusi key, jumlah worker dan cardinality cache yang berbeda dapat memberi angka berbeda.

## Stabilitas berkelanjutan

**Lulus.** Target traffic berjalan 600 detik; seluruh request dan proses klien selesai pada 617,79 detik, termasuk penyelesaian request yang tersisa. Tidak ada error HTTP, reconnect/error WebSocket, failure reload, panic, atau access log drop yang teramati.

Harness [fix_stability.py](fix_stability.py) menjalankan total offered-load 720 req/s pada HTTP/1, HTTPS, HTTP/2, cache, file statis dan gzip, ditambah 40 WebSocket jangka panjang (separuh TLS) serta koneksi WebSocket singkat bergantian. SIGHUP setiap 10 detik; penggantian bundle sertifikat atomik setiap 30 detik diverifikasi melalui fingerprint TLS dan dimuat oleh file watcher. Harness memeriksa respons, error, hasil echo, reload, log drop, RSS, thread dan FD.

| Ukuran | Hasil |
|---|---:|
| Request HTTP seluruh skenario | 432.000, seluruhnya status 200 |
| Echo WebSocket jangka panjang | 23.960 pesan, 0 mismatch/error/reconnect |
| Sesi WebSocket singkat berhasil | 1.198, 0 error |
| SIGHUP yang dikirim | 59 |
| Rotasi sertifikat teramati melalui fingerprint | 19 |
| Counter reload sukses akhir | 79, mencakup load awal, SIGHUP dan watcher |
| RSS median awal setelah warmup → akhir | 43.032 → 47.560 KiB (sekitar 42,0 → 46,4 MiB) |
| RSS maksimum setelah warmup | 47.728 KiB (46,6 MiB) |
| FD maksimum setelah warmup | 212 |
| Thread maksimum setelah warmup | 22 |

Jumlah request per jalur: HTTP 120.000, HTTPS 60.000, HTTP/2 60.000, cache 120.000, file statis 60.000 dan gzip 12.000. Detail respons/error setiap generator ada di folder [fix-stability](fix-stability); [hasil ringkas](fix-stability/results.json), [sampel resource](fix-stability/samples.json), [counter WebSocket](fix-stability/ws.csv), [metrics akhir](fix-stability/metrics.txt), dan [log harness](fix-stability.log) disimpan.

RSS bertambah sekitar 4,4 MiB dibanding awal setelah warmup, sedangkan FD tetap di sekitar 210. Resource tetap dalam batas harness selama pengujian ini; hasil tersebut tidak membuktikan ketiadaan leak pada seluruh durasi atau workload.

```sh
python3 docs/audit/fix_stability.py /path/to/release/conduit \
  --oha /path/to/oha --seconds 600 --out docs/audit/fix-stability
```

## Integritas dan batas kesimpulan

- Source `src/*.rs` dan manifest di build copy cocok dengan checkout hasil perbaikan. Hash binary dan source dicatat pada [fix-validation.json](fix-validation.json).
- Rust 1.99.1; fork Pingora pada putaran ini memakai gitlink `5c765c66590bdb7829ad32e00abc6f9a4e0410af`.
  Implementasi H2 berikutnya menambahkan patch lokal di atas gitlink yang sama. Native
  build sekarang memerlukan `scripts/prepare-vendor.sh`; lihat [vendor README](../../vendor/README.md).
- Hash artefak graph/source dokumentasi pada `graphify-resync.json` adalah snapshot putaran
  tersebut. Sinkronisasi dokumentasi terbaru dicatat di [documentation-sync.json](documentation-sync.json);
  hash implementasi final tetap ada di [http2-validation.json](http2-validation.json).
- Audit awal memakai graph yang sudah tersedia. Setelah patch, Graphify disinkronkan ulang melalui ekstraksi AST, pembaruan pemetaan dokumentasi, dan regenerasi JSON/HTML/report. Bukti sinkronisasi dicatat terpisah pada [graphify-resync.json](graphify-resync.json). Graph memetakan hubungan sumber; pembacaan source dan eksekusi binary tetap menentukan hasil closure.
- Context7 dipakai untuk memeriksa kontrak library, termasuk interval Tokio dan cache variance Pingora. Source fork lokal tetap menjadi rujukan untuk perilaku versi terkunci; dokumentasi branch main tidak otomatis membuktikan kesesuaian fork.
- Fallback files kini menolak semua symlink. Listing membutuhkan `/proc/self/fd`. Ini pembatasan kompatibilitas yang disengaja dan dicantumkan di dokumentasi konfigurasi.
- Variance raw-header dapat membuat entry tambahan untuk ejaan header yang sebenarnya ekuivalen. Pemisahan konservatif ini mencegah penyatuan representasi yang berbeda.
- Perbaikan ini menutup skenario temuan dan kontrol yang diuji. Tidak mengklaim audit setiap patch vendor, bebas semua bug/CVE, fuzz campaign baru, validasi MSRV, ataupun soak produksi 24 jam.
