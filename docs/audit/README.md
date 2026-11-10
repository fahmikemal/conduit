# Audit dan validasi Conduit

Diperbarui 10 November 2026 untuk implementasi commit `03cae2b`.
[README proyek](../../README.md), [referensi konfigurasi](../configuration.md) dan
[dokumentasi teknis](../technical.md) menjelaskan penggunaan saat ini.

## Status saat ini

Perbaikan F01/F02/F03/F05/F06/F07/F09, warning dan override wildcard F08, serta dokumentasi
F10 sudah diterapkan dan diuji ulang. F04 tetap ditarik sebagai bug: fresh cache bertahan
sesuai kontrak TTL/purge. HTTP/2 upstream juga sudah tersedia, dengan HTTP/1.1 tetap default.

| Hasil final | Bukti |
|---|---|
| 111 unit aplikasi lulus: 109 default + 2 pengukuran eksplisit | [Suite](http2-full-tests.log), [unit ignored](http2-ignored-unit.log) |
| 34 E2E lulus: 30 default + ACME, CrowdSec, slowloris, soak H2 | [Laporan final](HTTP2_UPSTREAM_VALIDATION.md#uji-fungsional-dan-regresi) |
| Lima E2E H2 diulang pada binary release final; satu regresi native pool Pingora lulus | [Release E2E](http2-final-release-e2e.log), [native](http2-pool-race.log) |
| Clippy `-D warnings` dan rustfmt lulus | [Clippy](http2-clippy.log), [fmt](http2-fmt.log) |
| PoC audit diulang pada release final, seluruh assertion closure/kontrol lulus | [Hasil closure final](http2-final-closure-results.json) |
| Benchmark jalur default: 6.000.000 request terukur, nol error | [Metode dan batas](HTTP2_UPSTREAM_VALIDATION.md#performa), [30 sampel](http2-default-performance-raw.jsonl) |
| Soak H2 lokal: 120 detik, 179.999 request sukses, 23 reload, FD 40 → 39 | [Hasil release](http2-release-stability-results.json) |

Hash binary release final: `474e0cbf143e0abbda86f1ff6ae42c9a2bfc13ed309f515d86dae381a7a41429`.
[http2-validation.json](http2-validation.json) mengikat hasil dengan source, manifest dan patch
Pingora yang diuji. Putaran pembaruan Markdown tidak mengubah implementasi aplikasi.

## Cara membaca laporan

| Dokumen | Cakupan |
|---|---|
| [HTTP2_UPSTREAM_VALIDATION.md](HTTP2_UPSTREAM_VALIDATION.md) | Hasil final sesudah fitur H2, pemakaian, backport, seluruh suite, benchmark dan soak H2. |
| [FIX_VALIDATION.md](FIX_VALIDATION.md) | Bukti penutupan temuan pada putaran sebelum H2: 111 unit, 28 E2E, benchmark dan soak campuran 600 detik. Hasilnya dilanjutkan oleh laporan H2. |
| [FINDINGS.md](FINDINGS.md) | Kondisi sebelum patch, prasyarat, akar masalah dan rancangan regresi. Status sekarang dicantumkan di awal. |
| [RECHECK.md](RECHECK.md) | Keputusan bug versus false positive pada binary sebelum patch; bukan daftar bug terbuka saat ini. |
| [WALKTHROUGH.md](WALKTHROUGH.md) | Ringkasan perubahan terkini diikuti bedah modul historis dengan nomor baris snapshot. |
| [INVENTORY.md](INVENTORY.md), [inventory.json](inventory.json) | Inventaris baseline `a6b2bb9`; angka/hash baseline dipertahankan dan penambahan penting dicatat terpisah. |
| [SOURCE.html](SOURCE.html) | Penjelajah source baseline bernomor baris; gunakan source checkout untuk implementasi sesudah perbaikan. |
| [recheck_runtime.py](recheck_runtime.py) | Harness HTTP/TLS yang dapat dijalankan pada binary target dengan `--expect-fixed`. |
| [reproduce.py](reproduce.py), [reproduction-results.json](reproduction-results.json) | Model demonstrasi awal; tidak setara dengan bukti eksekusi binary. |
| [Graphify report](../../graphify-out/GRAPH_REPORT.md), [graph interaktif](../../graphify-out/graph.html) | Pemetaan kode dan dokumentasi yang diperbarui sesudah perubahan. |

## Matriks penutupan

| ID | Status sekarang | Pengujian regresi |
|---|---|---|
| F01 | Decoding path satu kali diterapkan | `security_file_path_decoded_once` |
| F02 | Fallback berbasis descriptor dan listing terikat descriptor diterapkan | `security_file_fallback_containment` |
| F03 | Authority termasuk port masuk cache key | `security_cache_authority` |
| F04 | Ditarik sebagai bug; retensi fresh cache dipertahankan | Kontrol reload A → B dan purge pada harness closure |
| F05 | Overflow perkalian durasi ditolak | `security_duration_overflow_rejected` |
| F06 | Watch interval nol/tidak representable ditolak | `security_zero_watch_interval_rejected` |
| F07 | Admission untuk key yang sama diserialisasi | `security_rate_limit_concurrent_admission` |
| F08 | Override wildcard dipertegas dan coverage warning dibetulkan | `security_tls_wildcard_override_and_coverage` |
| F09 | Vary eksplisit mempertahankan coding/header field asli | `security_cache_unknown_encoding` |
| F10 | README selaras dengan pass-through Server origin | Existing E2E `server_header` |

Delapan tes `security_` ada di [tests/e2e.rs](../../tests/e2e.rs). Lima tes fungsional H2 dan
soak ada di [tests/support/h2_upstream.rs](../../tests/support/h2_upstream.rs). Regresi native
pool ikut disimpan dalam [patch Pingora](../../vendor/pingora-h2-reuse.patch).

## Reproduksi pada checkout baru

```sh
git submodule update --init --recursive
bash scripts/prepare-vendor.sh
cargo test --locked --all-targets
cargo test --locked --lib -- --ignored --nocapture
cargo test --locked --test e2e security_ -- --nocapture
cargo test --locked --test e2e upstream_http2_ -- --nocapture
cargo test --locked --test e2e acme_with_pebble -- --ignored --nocapture
cargo test --locked --test e2e crowdsec_bouncer_and_appsec -- --ignored --nocapture
cargo test --locked --test e2e slow_request_head_is_cut_off -- --ignored --nocapture
OHA_BIN=/path/to/oha cargo test --locked --test e2e upstream_http2_load_stability -- --ignored --nocapture
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all --check
```

ACME/CrowdSec memerlukan Docker atau wrapper Podman yang sesuai; slowloris sekitar satu
menit dan soak H2 sekitar dua menit. Empat E2E itu ignored secara default. Dua unit ignored
adalah pengukuran, bukan benchmark release. Keterangan kompatibilitas patch dan native test
berada di [vendor/README.md](../../vendor/README.md#tests).

## Graphify, Context7 dan provenance

Graphify memetakan hubungan, bukan membuktikan ketiadaan bug. AST aplikasi, dokumentasi aktif,
dan pemetaan tes diperbarui; source vendored Pingora dikecualikan oleh `.graphifyignore` dan
perubahannya didokumentasikan serta di-hash terpisah. Snapshot historis dan log mentah juga
dikecualikan agar status lama tidak menjadi fakta kode saat ini.

Context7 dipakai untuk kontrak library; dokumen Pingora branch main bukan jaminan perilaku
fork 0.9.1. Source fork terkunci dan hasil wire test menjadi rujukan perilaku aktual.
[Sinkronisasi H2](http2-graphify-resync.json) menyimpan snapshot graph sebelum pembaruan
Markdown; [documentation-sync.json](documentation-sync.json) mencatat cakupan, pemeriksaan
contoh/link, hash dokumen dan sinkronisasi graph pada putaran dokumentasi ini.

## Batas hasil

- Soak 24 jam Krynox yang disebut operator adalah run terpisah. Pengujian lokal ini tidak
  mengubah atau memverifikasi run tersebut, dan tidak mendeploy commit ke Krynox.
- Soak historis 8–9 November memakai build yang lebih lama. Soak final H2 berlangsung 120
  detik; RSS yang terukur tetap terbatas selama workload itu, bukan bukti bebas leak selamanya.
- Benchmark final membandingkan sebelum/sesudah fitur pada upstream HTTP/1.1, bukan peak H2
  versus H1. Host dipakai bersama dan affinity upstream rootless tidak ter-enforce.
- Putaran ini tidak mengulang full vendor suite, fuzz campaign, MSRV, replay log produksi,
  atau audit advisory dependency terkini. Angka line coverage tidak dihitung.
- Observasi lanjutan dalam laporan historis tetap merupakan pertanyaan desain/tes, bukan
  seluruhnya bug terkonfirmasi atau klaim sudah selesai.
