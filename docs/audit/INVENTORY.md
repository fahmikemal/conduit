# Inventaris file dan simbol

## Scope baseline dan penambahan saat ini

Indeks otomatis di bawah adalah snapshot **363 file** pada commit `a6b2bb9` sebelum
perbaikan. Jumlah baris, byte, simbol dan node graph dalam tabel bukan angka checkout
terbaru. Pada snapshot itu vendor gitlink belum berisi checkout lokal; kondisi tersebut
tidak menyatakan vendor masih kosong sekarang. Hash baseline ada di [inventory.json](inventory.json),
dan source bernomor baris ada di [SOURCE.html](SOURCE.html).

Implementasi saat ini adalah `03cae2b`: 29 modul `src/*.rs` tetap ada, delapan regresi
keamanan ditambahkan ke `tests/e2e.rs`, serta modul dan artefak berikut ditambahkan:

| File/kelompok | Penambahan penting |
|---|---|
| [tests/support/h2_upstream.rs](../../tests/support/h2_upstream.rs) | Lima tes H2 fungsional serta soak, origin H1/H2/TLS nyata dan klien gRPC. |
| [scripts/prepare-vendor.sh](../../scripts/prepare-vendor.sh) | Persiapan backport lokal yang idempotent. |
| [vendor/pingora-h2-reuse.patch](../../vendor/pingora-h2-reuse.patch) | Perbaikan race pool H2 dan tes native, di atas gitlink `5c765c6`. |
| [FIX_VALIDATION.md](FIX_VALIDATION.md) | Patch audit, 111 unit / 28 E2E pada putaran pertama dan soak campuran 600 detik. |
| [HTTP2_UPSTREAM_VALIDATION.md](HTTP2_UPSTREAM_VALIDATION.md) | Validasi final 111 unit / 34 E2E, 6 juta request A/B, soak H2 dan backport. |
| [http2-validation.json](http2-validation.json) | Hash source/binary/patch final dan hasil pengujian implementasi. |
| [documentation-sync.json](documentation-sync.json) | Daftar Markdown yang ditinjau, pemeriksaan link/contoh dan hash graph pada putaran dokumentasi. |

[Indeks audit](README.md) menyediakan status sekarang. Source vendored Pingora sengaja
berada di luar ekstraksi AST Graphify; rationale patch dipetakan melalui vendor README.
Tabel baseline dipertahankan agar reproduksi temuan lama tidak kehilangan konteks.

## Tabel file baseline

| File | Baris | Byte | Node graph | Jenis |
|---|---:|---:|---:|---|
| [.dockerignore](../../.dockerignore) | 6 | 55 | 0 | konfigurasi/dokumentasi/build/legal |
| [.gitattributes](../../.gitattributes) | 2 | 125 | 0 | konfigurasi/dokumentasi/build/legal |
| [.gitignore](../../.gitignore) | 12 | 260 | 0 | konfigurasi/dokumentasi/build/legal |
| [.gitmodules](../../.gitmodules) | 4 | 119 | 0 | konfigurasi/dokumentasi/build/legal |
| [.graphifyignore](../../.graphifyignore) | 10 | 346 | 0 | konfigurasi/dokumentasi/build/legal |
| [Cargo.lock](../../Cargo.lock) | 2828 | 68565 | 0 | konfigurasi/dokumentasi/build/legal |
| [Cargo.toml](../../Cargo.toml) | 96 | 4384 | 1 | konfigurasi/dokumentasi/build/legal |
| [Dockerfile](../../Dockerfile) | 34 | 1433 | 0 | konfigurasi/dokumentasi/build/legal |
| [LICENSE](../../LICENSE) | 201 | 11341 | 0 | konfigurasi/dokumentasi/build/legal |
| [NOTICE](../../NOTICE) | 9 | 401 | 0 | konfigurasi/dokumentasi/build/legal |
| [README.md](../../README.md) | 179 | 8072 | 7 | konfigurasi/dokumentasi/build/legal |
| [assets/brand/README.md](../../assets/brand/README.md) | 58 | 2949 | 20 | aset/dokumentasi brand |
| [assets/brand/conduit-app-icon.svg](../../assets/brand/conduit-app-icon.svg) | 9 | 517 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-horizontal-ember.svg](../../assets/brand/conduit-logo-horizontal-ember.svg) | 9 | 3009 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-horizontal-mono.svg](../../assets/brand/conduit-logo-horizontal-mono.svg) | 9 | 3029 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-horizontal-reverse.svg](../../assets/brand/conduit-logo-horizontal-reverse.svg) | 9 | 3009 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-horizontal.svg](../../assets/brand/conduit-logo-horizontal.svg) | 9 | 3009 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-stacked-ember.svg](../../assets/brand/conduit-logo-stacked-ember.svg) | 9 | 3042 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-stacked-mono.svg](../../assets/brand/conduit-logo-stacked-mono.svg) | 9 | 3062 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-stacked-reverse.svg](../../assets/brand/conduit-logo-stacked-reverse.svg) | 9 | 3042 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-logo-stacked.svg](../../assets/brand/conduit-logo-stacked.svg) | 9 | 3042 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-mark-ember.svg](../../assets/brand/conduit-mark-ember.svg) | 8 | 405 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-mark-mono.svg](../../assets/brand/conduit-mark-mono.svg) | 8 | 376 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-mark-reverse.svg](../../assets/brand/conduit-mark-reverse.svg) | 8 | 405 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-mark-small-reverse.svg](../../assets/brand/conduit-mark-small-reverse.svg) | 7 | 342 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-mark-small.svg](../../assets/brand/conduit-mark-small.svg) | 7 | 342 | 0 | aset/dokumentasi brand |
| [assets/brand/conduit-mark.svg](../../assets/brand/conduit-mark.svg) | 8 | 405 | 0 | aset/dokumentasi brand |
| [assets/brand/favicon.ico](../../assets/brand/favicon.ico) | biner | 15086 | 0 | aset/dokumentasi brand |
| [assets/brand/favicon.svg](../../assets/brand/favicon.svg) | 7 | 384 | 0 | aset/dokumentasi brand |
| [assets/brand/png/apple-touch-icon.png](../../assets/brand/png/apple-touch-icon.png) | biner | 4628 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-app-icon-1024.png](../../assets/brand/png/conduit-app-icon-1024.png) | biner | 44707 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-app-icon-512.png](../../assets/brand/png/conduit-app-icon-512.png) | biner | 20186 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-logo-horizontal-1200w.png](../../assets/brand/png/conduit-logo-horizontal-1200w.png) | biner | 31065 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-logo-horizontal-reverse-1200w.png](../../assets/brand/png/conduit-logo-horizontal-reverse-1200w.png) | biner | 28825 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-logo-stacked-1200w.png](../../assets/brand/png/conduit-logo-stacked-1200w.png) | biner | 67241 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-logo-stacked-reverse-1200w.png](../../assets/brand/png/conduit-logo-stacked-reverse-1200w.png) | biner | 62615 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-mark-512.png](../../assets/brand/png/conduit-mark-512.png) | biner | 24785 | 0 | aset/dokumentasi brand |
| [assets/brand/png/conduit-mark-reverse-512.png](../../assets/brand/png/conduit-mark-reverse-512.png) | biner | 22818 | 0 | aset/dokumentasi brand |
| [assets/brand/png/favicon-16.png](../../assets/brand/png/favicon-16.png) | biner | 529 | 0 | aset/dokumentasi brand |
| [assets/brand/png/favicon-32.png](../../assets/brand/png/favicon-32.png) | biner | 1036 | 0 | aset/dokumentasi brand |
| [conduit.example.yaml](../../conduit.example.yaml) | 473 | 27470 | 140 | konfigurasi/dokumentasi/build/legal |
| [deploy/conduit.logrotate](../../deploy/conduit.logrotate) | 13 | 331 | 0 | deployment/ingestion |
| [deploy/conduit.service](../../deploy/conduit.service) | 66 | 2341 | 0 | deployment/ingestion |
| [deploy/crowdsec/acquis-appsec.yaml](../../deploy/crowdsec/acquis-appsec.yaml) | 8 | 334 | 6 | deployment/ingestion |
| [deploy/crowdsec/acquis-conduit.yaml](../../deploy/crowdsec/acquis-conduit.yaml) | 5 | 146 | 4 | deployment/ingestion |
| [deploy/crowdsec/conduit-logs.yaml](../../deploy/crowdsec/conduit-logs.yaml) | 54 | 2214 | 11 | deployment/ingestion |
| [docs/configuration.md](../../docs/configuration.md) | 1749 | 89593 | 85 | konfigurasi/dokumentasi/build/legal |
| [docs/technical.md](../../docs/technical.md) | 959 | 82321 | 153 | konfigurasi/dokumentasi/build/legal |
| [fuzz/.gitignore](../../fuzz/.gitignore) | 5 | 48 | 0 | konfigurasi/dokumentasi/build/legal |
| [fuzz/Cargo.toml](../../fuzz/Cargo.toml) | 86 | 2085 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/fuzz_targets/access_log.rs](../../fuzz/fuzz_targets/access_log.rs) | 3 | 92 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/fuzz_targets/block_exploits.rs](../../fuzz/fuzz_targets/block_exploits.rs) | 3 | 96 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/fuzz_targets/body_replace.rs](../../fuzz/fuzz_targets/body_replace.rs) | 3 | 94 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/fuzz_targets/config.rs](../../fuzz/fuzz_targets/config.rs) | 3 | 88 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/fuzz_targets/path.rs](../../fuzz/fuzz_targets/path.rs) | 3 | 86 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/fuzz_targets/request.rs](../../fuzz/fuzz_targets/request.rs) | 3 | 89 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/fuzz_targets/route.rs](../../fuzz/fuzz_targets/route.rs) | 3 | 87 | 1 | konfigurasi/dokumentasi/build/legal |
| [fuzz/run.sh](../../fuzz/run.sh) | 22 | 845 | 2 | konfigurasi/dokumentasi/build/legal |
| [fuzz/seeds/access_log/0](../../fuzz/seeds/access_log/0) | biner | 64 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/dolibarr-conf-read](../../fuzz/seeds/block_exploits/dolibarr-conf-read) | 1 | 71 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s000](../../fuzz/seeds/block_exploits/s000) | 1 | 27 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s001](../../fuzz/seeds/block_exploits/s001) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s002](../../fuzz/seeds/block_exploits/s002) | 1 | 18 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s003](../../fuzz/seeds/block_exploits/s003) | 1 | 75 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s004](../../fuzz/seeds/block_exploits/s004) | 1 | 32 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s005](../../fuzz/seeds/block_exploits/s005) | 1 | 25 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s006](../../fuzz/seeds/block_exploits/s006) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s007](../../fuzz/seeds/block_exploits/s007) | 1 | 12 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s008](../../fuzz/seeds/block_exploits/s008) | 1 | 3 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s009](../../fuzz/seeds/block_exploits/s009) | 1 | 1 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s010](../../fuzz/seeds/block_exploits/s010) | 1 | 24 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s011](../../fuzz/seeds/block_exploits/s011) | 1 | 166 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s012](../../fuzz/seeds/block_exploits/s012) | 1 | 7 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s013](../../fuzz/seeds/block_exploits/s013) | 1 | 10 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s014](../../fuzz/seeds/block_exploits/s014) | 1 | 10 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s015](../../fuzz/seeds/block_exploits/s015) | 1 | 5 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s016](../../fuzz/seeds/block_exploits/s016) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s017](../../fuzz/seeds/block_exploits/s017) | 1 | 14 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s018](../../fuzz/seeds/block_exploits/s018) | 1 | 20 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s019](../../fuzz/seeds/block_exploits/s019) | 1 | 5 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s020](../../fuzz/seeds/block_exploits/s020) | 1 | 16 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s021](../../fuzz/seeds/block_exploits/s021) | 1 | 14 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s022](../../fuzz/seeds/block_exploits/s022) | 1 | 5 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s023](../../fuzz/seeds/block_exploits/s023) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s024](../../fuzz/seeds/block_exploits/s024) | 1 | 10 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s025](../../fuzz/seeds/block_exploits/s025) | 1 | 25 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s026](../../fuzz/seeds/block_exploits/s026) | 1 | 10 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s027](../../fuzz/seeds/block_exploits/s027) | 1 | 12 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s028](../../fuzz/seeds/block_exploits/s028) | 1 | 13 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s029](../../fuzz/seeds/block_exploits/s029) | 1 | 37 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s030](../../fuzz/seeds/block_exploits/s030) | 1 | 33 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s031](../../fuzz/seeds/block_exploits/s031) | 1 | 25 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s032](../../fuzz/seeds/block_exploits/s032) | 1 | 10 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s033](../../fuzz/seeds/block_exploits/s033) | 1 | 45 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s034](../../fuzz/seeds/block_exploits/s034) | 1 | 52 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s035](../../fuzz/seeds/block_exploits/s035) | 1 | 31 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s036](../../fuzz/seeds/block_exploits/s036) | 1 | 33 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s037](../../fuzz/seeds/block_exploits/s037) | 1 | 33 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s038](../../fuzz/seeds/block_exploits/s038) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s039](../../fuzz/seeds/block_exploits/s039) | 1 | 42 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s040](../../fuzz/seeds/block_exploits/s040) | 1 | 6 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s041](../../fuzz/seeds/block_exploits/s041) | 1 | 11 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s042](../../fuzz/seeds/block_exploits/s042) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s043](../../fuzz/seeds/block_exploits/s043) | 1 | 51 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s044](../../fuzz/seeds/block_exploits/s044) | 1 | 22 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s045](../../fuzz/seeds/block_exploits/s045) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s046](../../fuzz/seeds/block_exploits/s046) | 1 | 82 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s047](../../fuzz/seeds/block_exploits/s047) | 1 | 25 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s048](../../fuzz/seeds/block_exploits/s048) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s049](../../fuzz/seeds/block_exploits/s049) | 1 | 62 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s050](../../fuzz/seeds/block_exploits/s050) | 1 | 45 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s051](../../fuzz/seeds/block_exploits/s051) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s052](../../fuzz/seeds/block_exploits/s052) | 1 | 90 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s053](../../fuzz/seeds/block_exploits/s053) | 1 | 35 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s054](../../fuzz/seeds/block_exploits/s054) | 1 | 230 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s055](../../fuzz/seeds/block_exploits/s055) | 1 | 58 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s056](../../fuzz/seeds/block_exploits/s056) | 1 | 15 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s057](../../fuzz/seeds/block_exploits/s057) | 1 | 10 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s058](../../fuzz/seeds/block_exploits/s058) | 1 | 68 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s059](../../fuzz/seeds/block_exploits/s059) | 1 | 65 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s060](../../fuzz/seeds/block_exploits/s060) | 1 | 30 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s061](../../fuzz/seeds/block_exploits/s061) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s062](../../fuzz/seeds/block_exploits/s062) | 1 | 42 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s063](../../fuzz/seeds/block_exploits/s063) | 1 | 42 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s064](../../fuzz/seeds/block_exploits/s064) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s065](../../fuzz/seeds/block_exploits/s065) | 1 | 39 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s066](../../fuzz/seeds/block_exploits/s066) | 1 | 26 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s067](../../fuzz/seeds/block_exploits/s067) | 1 | 143 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s068](../../fuzz/seeds/block_exploits/s068) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s069](../../fuzz/seeds/block_exploits/s069) | 1 | 51 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s070](../../fuzz/seeds/block_exploits/s070) | 1 | 29 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s071](../../fuzz/seeds/block_exploits/s071) | 1 | 62 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s072](../../fuzz/seeds/block_exploits/s072) | 1 | 31 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s073](../../fuzz/seeds/block_exploits/s073) | 1 | 42 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s074](../../fuzz/seeds/block_exploits/s074) | 1 | 74 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s075](../../fuzz/seeds/block_exploits/s075) | 1 | 63 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s076](../../fuzz/seeds/block_exploits/s076) | 1 | 70 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s077](../../fuzz/seeds/block_exploits/s077) | 1 | 13 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s078](../../fuzz/seeds/block_exploits/s078) | 1 | 27 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s079](../../fuzz/seeds/block_exploits/s079) | 1 | 23 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s080](../../fuzz/seeds/block_exploits/s080) | 1 | 27 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s081](../../fuzz/seeds/block_exploits/s081) | 1 | 70 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s082](../../fuzz/seeds/block_exploits/s082) | 1 | 48 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s083](../../fuzz/seeds/block_exploits/s083) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s084](../../fuzz/seeds/block_exploits/s084) | 1 | 79 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s085](../../fuzz/seeds/block_exploits/s085) | 1 | 27 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s086](../../fuzz/seeds/block_exploits/s086) | 1 | 69 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s087](../../fuzz/seeds/block_exploits/s087) | 1 | 103 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s088](../../fuzz/seeds/block_exploits/s088) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s089](../../fuzz/seeds/block_exploits/s089) | 1 | 30 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s090](../../fuzz/seeds/block_exploits/s090) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s091](../../fuzz/seeds/block_exploits/s091) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s092](../../fuzz/seeds/block_exploits/s092) | 1 | 89 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s093](../../fuzz/seeds/block_exploits/s093) | 1 | 24 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s094](../../fuzz/seeds/block_exploits/s094) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s095](../../fuzz/seeds/block_exploits/s095) | 1 | 50 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s096](../../fuzz/seeds/block_exploits/s096) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s097](../../fuzz/seeds/block_exploits/s097) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s098](../../fuzz/seeds/block_exploits/s098) | 1 | 58 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s099](../../fuzz/seeds/block_exploits/s099) | 1 | 41 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s100](../../fuzz/seeds/block_exploits/s100) | 1 | 21 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s101](../../fuzz/seeds/block_exploits/s101) | 1 | 35 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s102](../../fuzz/seeds/block_exploits/s102) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s103](../../fuzz/seeds/block_exploits/s103) | 1 | 42 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s104](../../fuzz/seeds/block_exploits/s104) | 1 | 19 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s105](../../fuzz/seeds/block_exploits/s105) | 1 | 19 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s106](../../fuzz/seeds/block_exploits/s106) | 1 | 47 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s107](../../fuzz/seeds/block_exploits/s107) | 1 | 50 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s108](../../fuzz/seeds/block_exploits/s108) | 1 | 24 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s109](../../fuzz/seeds/block_exploits/s109) | 1 | 45 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s110](../../fuzz/seeds/block_exploits/s110) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s111](../../fuzz/seeds/block_exploits/s111) | 1 | 50 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s112](../../fuzz/seeds/block_exploits/s112) | 1 | 23 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s113](../../fuzz/seeds/block_exploits/s113) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s114](../../fuzz/seeds/block_exploits/s114) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s115](../../fuzz/seeds/block_exploits/s115) | 1 | 27 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s116](../../fuzz/seeds/block_exploits/s116) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s117](../../fuzz/seeds/block_exploits/s117) | 1 | 52 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s118](../../fuzz/seeds/block_exploits/s118) | 1 | 36 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s119](../../fuzz/seeds/block_exploits/s119) | 1 | 27 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s120](../../fuzz/seeds/block_exploits/s120) | 1 | 41 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s121](../../fuzz/seeds/block_exploits/s121) | 1 | 29 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s122](../../fuzz/seeds/block_exploits/s122) | 1 | 32 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s123](../../fuzz/seeds/block_exploits/s123) | 1 | 56 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s124](../../fuzz/seeds/block_exploits/s124) | 1 | 77 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s125](../../fuzz/seeds/block_exploits/s125) | 1 | 62 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s126](../../fuzz/seeds/block_exploits/s126) | 1 | 20 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s127](../../fuzz/seeds/block_exploits/s127) | 1 | 43 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s128](../../fuzz/seeds/block_exploits/s128) | 1 | 51 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s129](../../fuzz/seeds/block_exploits/s129) | 1 | 62 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s130](../../fuzz/seeds/block_exploits/s130) | 1 | 33 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s131](../../fuzz/seeds/block_exploits/s131) | 1 | 70 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s132](../../fuzz/seeds/block_exploits/s132) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s133](../../fuzz/seeds/block_exploits/s133) | 1 | 38 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s134](../../fuzz/seeds/block_exploits/s134) | 1 | 45 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s135](../../fuzz/seeds/block_exploits/s135) | 1 | 30 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s136](../../fuzz/seeds/block_exploits/s136) | 1 | 44 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s137](../../fuzz/seeds/block_exploits/s137) | 1 | 21 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s138](../../fuzz/seeds/block_exploits/s138) | 1 | 29 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s139](../../fuzz/seeds/block_exploits/s139) | 1 | 140 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s140](../../fuzz/seeds/block_exploits/s140) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s141](../../fuzz/seeds/block_exploits/s141) | 1 | 47 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s142](../../fuzz/seeds/block_exploits/s142) | 1 | 38 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s143](../../fuzz/seeds/block_exploits/s143) | 1 | 53 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s144](../../fuzz/seeds/block_exploits/s144) | 1 | 56 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s145](../../fuzz/seeds/block_exploits/s145) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s146](../../fuzz/seeds/block_exploits/s146) | 1 | 49 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s147](../../fuzz/seeds/block_exploits/s147) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s148](../../fuzz/seeds/block_exploits/s148) | 1 | 51 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s149](../../fuzz/seeds/block_exploits/s149) | 1 | 36 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s150](../../fuzz/seeds/block_exploits/s150) | 1 | 39 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s151](../../fuzz/seeds/block_exploits/s151) | 1 | 55 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s152](../../fuzz/seeds/block_exploits/s152) | 1 | 44 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s153](../../fuzz/seeds/block_exploits/s153) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s154](../../fuzz/seeds/block_exploits/s154) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s155](../../fuzz/seeds/block_exploits/s155) | 1 | 52 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s156](../../fuzz/seeds/block_exploits/s156) | 1 | 39 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s157](../../fuzz/seeds/block_exploits/s157) | 1 | 18 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s158](../../fuzz/seeds/block_exploits/s158) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s159](../../fuzz/seeds/block_exploits/s159) | 1 | 45 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s160](../../fuzz/seeds/block_exploits/s160) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s161](../../fuzz/seeds/block_exploits/s161) | 1 | 43 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s162](../../fuzz/seeds/block_exploits/s162) | 1 | 24 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s163](../../fuzz/seeds/block_exploits/s163) | 1 | 53 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s164](../../fuzz/seeds/block_exploits/s164) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s165](../../fuzz/seeds/block_exploits/s165) | 1 | 25 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s166](../../fuzz/seeds/block_exploits/s166) | 1 | 38 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s167](../../fuzz/seeds/block_exploits/s167) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s168](../../fuzz/seeds/block_exploits/s168) | 1 | 61 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s169](../../fuzz/seeds/block_exploits/s169) | 1 | 44 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s170](../../fuzz/seeds/block_exploits/s170) | 1 | 51 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s171](../../fuzz/seeds/block_exploits/s171) | 1 | 16 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s172](../../fuzz/seeds/block_exploits/s172) | 1 | 16 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s173](../../fuzz/seeds/block_exploits/s173) | 1 | 48 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s174](../../fuzz/seeds/block_exploits/s174) | 1 | 43 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s175](../../fuzz/seeds/block_exploits/s175) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s176](../../fuzz/seeds/block_exploits/s176) | 1 | 36 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s177](../../fuzz/seeds/block_exploits/s177) | 1 | 38 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s178](../../fuzz/seeds/block_exploits/s178) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s179](../../fuzz/seeds/block_exploits/s179) | 1 | 46 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s180](../../fuzz/seeds/block_exploits/s180) | 1 | 45 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s181](../../fuzz/seeds/block_exploits/s181) | 1 | 35 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s182](../../fuzz/seeds/block_exploits/s182) | 1 | 27 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s183](../../fuzz/seeds/block_exploits/s183) | 1 | 16 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s184](../../fuzz/seeds/block_exploits/s184) | 1 | 35 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s185](../../fuzz/seeds/block_exploits/s185) | 1 | 23 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s186](../../fuzz/seeds/block_exploits/s186) | 1 | 44 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s187](../../fuzz/seeds/block_exploits/s187) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s188](../../fuzz/seeds/block_exploits/s188) | 1 | 23 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s189](../../fuzz/seeds/block_exploits/s189) | 1 | 28 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s190](../../fuzz/seeds/block_exploits/s190) | 1 | 22 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s191](../../fuzz/seeds/block_exploits/s191) | 1 | 38 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s192](../../fuzz/seeds/block_exploits/s192) | 1 | 54 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s193](../../fuzz/seeds/block_exploits/s193) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s194](../../fuzz/seeds/block_exploits/s194) | 1 | 51 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s195](../../fuzz/seeds/block_exploits/s195) | 1 | 57 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s196](../../fuzz/seeds/block_exploits/s196) | 1 | 29 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s197](../../fuzz/seeds/block_exploits/s197) | 1 | 34 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s198](../../fuzz/seeds/block_exploits/s198) | 1 | 35 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s199](../../fuzz/seeds/block_exploits/s199) | 1 | 66 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s200](../../fuzz/seeds/block_exploits/s200) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s201](../../fuzz/seeds/block_exploits/s201) | 1 | 44 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s202](../../fuzz/seeds/block_exploits/s202) | 1 | 32 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s203](../../fuzz/seeds/block_exploits/s203) | 1 | 33 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s204](../../fuzz/seeds/block_exploits/s204) | 1 | 24 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s205](../../fuzz/seeds/block_exploits/s205) | 1 | 14 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s206](../../fuzz/seeds/block_exploits/s206) | 1 | 76 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s207](../../fuzz/seeds/block_exploits/s207) | 1 | 67 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s208](../../fuzz/seeds/block_exploits/s208) | 1 | 51 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s209](../../fuzz/seeds/block_exploits/s209) | 1 | 30 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s210](../../fuzz/seeds/block_exploits/s210) | 1 | 23 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s211](../../fuzz/seeds/block_exploits/s211) | 1 | 43 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s212](../../fuzz/seeds/block_exploits/s212) | 1 | 67 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s213](../../fuzz/seeds/block_exploits/s213) | 1 | 54 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s214](../../fuzz/seeds/block_exploits/s214) | 1 | 36 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s215](../../fuzz/seeds/block_exploits/s215) | 1 | 18 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s216](../../fuzz/seeds/block_exploits/s216) | 1 | 21 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s217](../../fuzz/seeds/block_exploits/s217) | 1 | 63 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s218](../../fuzz/seeds/block_exploits/s218) | 1 | 17 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s219](../../fuzz/seeds/block_exploits/s219) | 1 | 26 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s220](../../fuzz/seeds/block_exploits/s220) | 1 | 18 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s221](../../fuzz/seeds/block_exploits/s221) | 1 | 79 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s222](../../fuzz/seeds/block_exploits/s222) | 1 | 21 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s223](../../fuzz/seeds/block_exploits/s223) | 1 | 56 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s224](../../fuzz/seeds/block_exploits/s224) | 1 | 19 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s225](../../fuzz/seeds/block_exploits/s225) | 1 | 62 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s226](../../fuzz/seeds/block_exploits/s226) | 1 | 11 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s227](../../fuzz/seeds/block_exploits/s227) | 1 | 35 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s228](../../fuzz/seeds/block_exploits/s228) | 1 | 15 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s229](../../fuzz/seeds/block_exploits/s229) | 1 | 16 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s230](../../fuzz/seeds/block_exploits/s230) | 1 | 22 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/s231](../../fuzz/seeds/block_exploits/s231) | 1 | 40 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/block_exploits/ua-turnitin](../../fuzz/seeds/block_exploits/ua-turnitin) | 1 | 57 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/body_replace/1](../../fuzz/seeds/body_replace/1) | biner | 24 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/body_replace/2](../../fuzz/seeds/body_replace/2) | biner | 38 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/config/example.yaml](../../fuzz/seeds/config/example.yaml) | 412 | 22771 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/config/routes.yaml](../../fuzz/seeds/config/routes.yaml) | 61 | 1966 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/path/0](../../fuzz/seeds/path/0) | 1 | 1 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/path/1](../../fuzz/seeds/path/1) | 1 | 13 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/path/2](../../fuzz/seeds/path/2) | 1 | 9 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/path/3](../../fuzz/seeds/path/3) | 1 | 6 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/path/4](../../fuzz/seeds/path/4) | 1 | 6 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/path/5](../../fuzz/seeds/path/5) | 1 | 4 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/app](../../fuzz/seeds/request/app) | 5 | 104 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/exploits](../../fuzz/seeds/request/exploits) | 4 | 126 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/exploits2](../../fuzz/seeds/request/exploits2) | 3 | 94 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/git](../../fuzz/seeds/request/git) | 3 | 66 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/pb](../../fuzz/seeds/request/pb) | 5 | 169 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/route1](../../fuzz/seeds/request/route1) | biner | 58 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/route2](../../fuzz/seeds/request/route2) | biner | 35 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/shop](../../fuzz/seeds/request/shop) | 4 | 116 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/request/sso](../../fuzz/seeds/request/sso) | 3 | 58 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/route/1](../../fuzz/seeds/route/1) | 1 | 19 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/route/2](../../fuzz/seeds/route/2) | 1 | 10 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/route/3](../../fuzz/seeds/route/3) | 1 | 15 | 0 | input seed (data, bukan assertion) |
| [fuzz/seeds/route/query-refused](../../fuzz/seeds/route/query-refused) | biner | 19 | 0 | input seed (data, bukan assertion) |
| [graphify-out/GRAPH_REPORT.md](../../graphify-out/GRAPH_REPORT.md) | 688 | 31476 | 0 | artefak graph turunan |
| [graphify-out/graph.html](../../graphify-out/graph.html) | 474 | 1996433 | 0 | artefak graph turunan |
| [graphify-out/graph.json](../../graphify-out/graph.json) | 1 | 1826609 | 0 | artefak graph turunan |
| [mise.toml](../../mise.toml) | 44 | 1551 | 0 | konfigurasi/dokumentasi/build/legal |
| [scripts/ab.sh](../../scripts/ab.sh) | 189 | 8298 | 7 | tooling/harness |
| [scripts/bench.sh](../../scripts/bench.sh) | 344 | 15947 | 10 | tooling/harness |
| [scripts/build-release.sh](../../scripts/build-release.sh) | 21 | 747 | 2 | tooling/harness |
| [scripts/compare.py](../../scripts/compare.py) | 623 | 26886 | 27 | tooling/harness |
| [scripts/parity.py](../../scripts/parity.py) | 210 | 8291 | 8 | tooling/harness |
| [scripts/parity/stub.py](../../scripts/parity/stub.py) | 57 | 1797 | 7 | tooling/harness |
| [scripts/pgo-train.sh](../../scripts/pgo-train.sh) | 168 | 7135 | 5 | tooling/harness |
| [scripts/pgo.sh](../../scripts/pgo.sh) | 63 | 2658 | 3 | tooling/harness |
| [scripts/soak/deploy.sh](../../scripts/soak/deploy.sh) | 36 | 1990 | 2 | tooling/harness |
| [scripts/soak/report.py](../../scripts/soak/report.py) | 231 | 10295 | 6 | tooling/harness |
| [scripts/soak/soak.sh](../../scripts/soak/soak.sh) | 263 | 10344 | 11 | tooling/harness |
| [scripts/soak/ws.py](../../scripts/soak/ws.py) | 224 | 7458 | 16 | tooling/harness |
| [src/accesslog.rs](../../src/accesslog.rs) | 807 | 29267 | 53 | implementasi Rust / embedded unit tests |
| [src/acme.rs](../../src/acme.rs) | 928 | 34170 | 53 | implementasi Rust / embedded unit tests |
| [src/admin.rs](../../src/admin.rs) | 274 | 10253 | 10 | implementasi Rust / embedded unit tests |
| [src/auth.rs](../../src/auth.rs) | 651 | 22890 | 44 | implementasi Rust / embedded unit tests |
| [src/cache.rs](../../src/cache.rs) | 863 | 30164 | 68 | implementasi Rust / embedded unit tests |
| [src/config.rs](../../src/config.rs) | 1528 | 55343 | 137 | implementasi Rust / embedded unit tests |
| [src/crowdsec.rs](../../src/crowdsec.rs) | 1483 | 49512 | 120 | implementasi Rust / embedded unit tests |
| [src/exploits.rs](../../src/exploits.rs) | 2390 | 79191 | 86 | implementasi Rust / embedded unit tests |
| [src/files.rs](../../src/files.rs) | 1271 | 45379 | 80 | implementasi Rust / embedded unit tests |
| [src/fuzzing.rs](../../src/fuzzing.rs) | 493 | 17593 | 11 | implementasi Rust / embedded unit tests |
| [src/httpc.rs](../../src/httpc.rs) | 202 | 7775 | 8 | implementasi Rust / embedded unit tests |
| [src/lib.rs](../../src/lib.rs) | 34 | 740 | 1 | implementasi Rust / embedded unit tests |
| [src/limits.rs](../../src/limits.rs) | 468 | 14745 | 32 | implementasi Rust / embedded unit tests |
| [src/logger.rs](../../src/logger.rs) | 128 | 4118 | 9 | implementasi Rust / embedded unit tests |
| [src/main.rs](../../src/main.rs) | 297 | 10060 | 13 | implementasi Rust / embedded unit tests |
| [src/metrics.rs](../../src/metrics.rs) | 342 | 12407 | 14 | implementasi Rust / embedded unit tests |
| [src/pathnorm.rs](../../src/pathnorm.rs) | 118 | 3515 | 9 | implementasi Rust / embedded unit tests |
| [src/plugin.rs](../../src/plugin.rs) | 573 | 18958 | 61 | implementasi Rust / embedded unit tests |
| [src/proxy.rs](../../src/proxy.rs) | 2033 | 74798 | 94 | implementasi Rust / embedded unit tests |
| [src/realip.rs](../../src/realip.rs) | 205 | 6345 | 14 | implementasi Rust / embedded unit tests |
| [src/reload.rs](../../src/reload.rs) | 219 | 8239 | 10 | implementasi Rust / embedded unit tests |
| [src/replace.rs](../../src/replace.rs) | 324 | 11617 | 21 | implementasi Rust / embedded unit tests |
| [src/route.rs](../../src/route.rs) | 259 | 9141 | 18 | implementasi Rust / embedded unit tests |
| [src/rules.rs](../../src/rules.rs) | 481 | 16064 | 48 | implementasi Rust / embedded unit tests |
| [src/runtime.rs](../../src/runtime.rs) | 874 | 30056 | 26 | implementasi Rust / embedded unit tests |
| [src/timefmt.rs](../../src/timefmt.rs) | 94 | 2771 | 8 | implementasi Rust / embedded unit tests |
| [src/tls.rs](../../src/tls.rs) | 627 | 23565 | 38 | implementasi Rust / embedded unit tests |
| [src/units.rs](../../src/units.rs) | 121 | 3790 | 13 | implementasi Rust / embedded unit tests |
| [src/upstream.rs](../../src/upstream.rs) | 838 | 29076 | 62 | implementasi Rust / embedded unit tests |
| [tests/e2e.rs](../../tests/e2e.rs) | 5160 | 179768 | 110 | integration test dan fixture |
| [vendor/README.md](../../vendor/README.md) | 304 | 19092 | 45 | konfigurasi/dokumentasi/build/legal |

## Simbol Rust dan test

Deklarasi dideteksi regex perbaris; macro, signature multiline dan nama metode sama tidak diselesaikan sebagai AST. Posisi adalah titik navigasi, bukan bukti call graph.

### src/accesslog.rs

807 baris; 5 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 36 | struct | `Entry` |
| 86 | fn | `is_zero` |
| 90 | fn | `is_zero_u32` |
| 94 | fn | `ser_ts` |
| 99 | fn | `ser_millis` |
| 103 | fn | `ser_opt_millis` |
| 114 | fn | `write_json` |
| 198 | mod | `json` |
| 204 | const | `ESCAPE` |
| 221 | const | `HEX` |
| 222 | const | `ONES` |
| 223 | const | `HIGHS` |
| 228 | fn | `needs_escape` |
| 235 | fn | `str` |
| 265 | fn | `int` |
| 270 | fn | `ip` |
| 292 | fn | `millis` |
| 321 | struct | `Shard` |
| 326 | struct | `Shards` |
| 333 | fn | `lock` |
| 339 | static | `MINE` |
| 342 | struct | `AccessLog` |
| 353 | fn | `new` |
| 354 | static | `NEXT_ID` |
| 376 | fn | `enabled` |
| 380 | fn | `path` |
| 384 | fn | `write` |
| 408 | fn | `start` |
| 428 | fn | `flush` |
| 437 | struct | `OpenFile` |
| 443 | struct | `Writer` |
| 455 | const | `POLL` |
| 456 | const | `FLUSH_EVERY` |
| 457 | const | `REOPEN_CHECK_EVERY` |
| 460 | fn | `new` |
| 472 | fn | `open` |
| 488 | fn | `run` |
| 522 | fn | `drain` |
| 543 | fn | `append` |
| 566 | fn | `flush` |
| 576 | fn | `reopen_rotated` |
| 586 | fn | `report_drops` |
| 599 | mod | `tests` |
| 603 | fn | `entry` |
| 633 | fn | `line` |
| 640 | fn | `json_line` |
| 648 | fn | `hostile_values_stay_one_line` |
| 657 | struct | `Rng` |
| 660 | fn | `next` |
| 667 | fn | `below` |
| 673 | fn | `text` |
| 674 | const | `PIECES` |
| 692 | fn | `duration` |
| 704 | fn | `ip` |
| 722 | fn | `write_json_matches_serde_json` |
| 768 | fn | `full_shard_drops_and_counts` |
| 781 | fn | `writer_appends_and_follows_rotation` |

### src/acme.rs

928 baris; 6 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 50 | static | `CHALLENGES` |
| 54 | fn | `challenge_answer` |
| 61 | struct | `Published` |
| 64 | fn | `add` |
| 74 | fn | `drop` |
| 84 | struct | `SiteStatus` |
| 96 | struct | `Status` |
| 100 | fn | `unix` |
| 105 | fn | `at` |
| 112 | const | `HTTP_TIMEOUT` |
| 114 | const | `MAX_RESPONSE` |
| 118 | struct | `Http` |
| 121 | fn | `new` |
| 130 | fn | `send` |
| 163 | fn | `request` |
| 176 | fn | `write_private` |
| 201 | struct | `StorageLock` |
| 203 | fn | `try_lock` |
| 228 | struct | `Stored` |
| 237 | fn | `asn1_unix` |
| 243 | fn | `cert_id` |
| 252 | fn | `inspect` |
| 271 | fn | `lifetime_renewal` |
| 277 | fn | `random_unit` |
| 288 | fn | `new_key` |
| 294 | fn | `csr` |
| 315 | fn | `bundle` |
| 335 | fn | `acme_err` |
| 340 | fn | `failure_detail` |
| 365 | fn | `order` |
| 455 | fn | `backoff` |
| 461 | struct | `Ari` |
| 468 | struct | `SiteState` |
| 474 | struct | `Connected` |
| 482 | struct | `Manager` |
| 487 | enum | `Due` |
| 494 | fn | `account` |
| 541 | fn | `due` |
| 604 | fn | `tick` |
| 684 | fn | `issue` |
| 725 | struct | `AcmeService` |
| 730 | fn | `new` |
| 737 | fn | `start` |
| 758 | mod | `tests` |
| 764 | fn | `challenge_tokens_are_withdrawn` |
| 778 | fn | `renewal_from_lifetime` |
| 799 | fn | `backoff_grows_and_caps` |
| 808 | fn | `issued` |
| 870 | fn | `stored_bundle_loads_and_identifies` |
| 907 | fn | `csr_names_every_host` |
| 918 | fn | `storage_lock_is_exclusive` |

### src/admin.rs

274 baris; 0 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 21 | struct | `Admin` |
| 25 | fn | `service` |
| 33 | fn | `unix` |
| 37 | fn | `reply` |
| 48 | fn | `status` |
| 178 | fn | `purge` |
| 211 | fn | `bad` |
| 220 | fn | `unescape` |
| 241 | fn | `response` |

### src/auth.rs

651 baris; 4 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 26 | type | `Hasher` |
| 29 | struct | `Counters` |
| 36 | fn | `new` |
| 46 | fn | `refused` |
| 52 | fn | `warn_sometimes` |
| 53 | static | `START` |
| 62 | const | `VERIFIED_MAX` |
| 66 | static | `HASHING` |
| 72 | const | `HASHING_WAIT` |
| 76 | static | `CACHE_KEY` |
| 82 | struct | `BasicAuth` |
| 94 | fn | `credentials` |
| 106 | fn | `cache_key` |
| 114 | fn | `verify` |
| 129 | fn | `new` |
| 194 | fn | `challenge` |
| 208 | fn | `name` |
| 212 | fn | `needs_path` |
| 216 | fn | `gate` |
| 233 | fn | `check` |
| 267 | const | `FORWARD_MAX_BODY` |
| 269 | struct | `ForwardAuth` |
| 279 | static | `X_FORWARDED_METHOD` |
| 280 | static | `X_FORWARDED_URI` |
| 281 | static | `X_FORWARDED_HOST` |
| 282 | static | `X_FORWARDED_PROTO` |
| 283 | static | `X_FORWARDED_FOR` |
| 287 | fn | `passed_to_auth` |
| 307 | fn | `passed_to_client` |
| 322 | fn | `new` |
| 349 | fn | `request_headers` |
| 397 | fn | `name` |
| 401 | fn | `needs_path` |
| 405 | fn | `gate` |
| 413 | fn | `check` |
| 458 | mod | `tests` |
| 463 | fn | `req` |
| 471 | fn | `view` |
| 484 | fn | `basic` |
| 492 | fn | `credentials_parsing` |
| 511 | fn | `config` |
| 524 | fn | `basic_auth_settings` |
| 551 | fn | `check` |
| 561 | fn | `basic_auth_flow` |
| 602 | fn | `forward_auth_request` |

### src/cache.rs

863 baris; 4 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 44 | const | `SHARDS` |
| 46 | const | `ENTRY_OVERHEAD` |
| 49 | struct | `SiteCache` |
| 55 | fn | `no_default_ttl` |
| 60 | fn | `compile` |
| 75 | fn | `takes` |
| 83 | fn | `cacheable` |
| 121 | fn | `heuristic` |
| 128 | fn | `vary_names` |
| 139 | fn | `variance` |
| 186 | struct | `Tag` |
| 191 | fn | `push_framed` |
| 198 | fn | `key` |
| 215 | fn | `outcome` |
| 227 | const | `RESULTS` |
| 236 | fn | `result_index` |
| 242 | static | `STORE` |
| 243 | static | `EVICTION` |
| 249 | static | `PREDICTOR` |
| 251 | static | `LOCK_AGE` |
| 252 | static | `LOCK` |
| 256 | fn | `lock` |
| 261 | fn | `apply` |
| 267 | struct | `Stats` |
| 273 | fn | `stats` |
| 283 | struct | `Object` |
| 293 | fn | `entry` |
| 298 | type | `Shard` |
| 300 | struct | `Store` |
| 307 | fn | `lock_shard` |
| 312 | fn | `new` |
| 321 | fn | `shard` |
| 325 | fn | `get` |
| 329 | fn | `forget` |
| 337 | fn | `insert` |
| 347 | fn | `rewrite` |
| 373 | fn | `purge_where` |
| 392 | fn | `purge` |
| 401 | fn | `retain_sites` |
| 407 | fn | `lookup` |
| 430 | fn | `get_miss_handler` |
| 453 | fn | `purge` |
| 478 | fn | `expire` |
| 496 | fn | `update_meta` |
| 506 | fn | `as_any` |
| 511 | struct | `Hit` |
| 522 | fn | `read_body` |
| 530 | fn | `finish` |
| 539 | fn | `can_seek` |
| 543 | fn | `seek` |
| 557 | fn | `get_eviction_weight` |
| 561 | fn | `entry_id` |
| 565 | fn | `as_any` |
| 569 | fn | `as_any_mut` |
| 574 | struct | `Miss` |
| 586 | fn | `write_body` |
| 591 | fn | `finish` |
| 609 | fn | `entry_id` |
| 616 | fn | `tracked` |
| 621 | mod | `tests` |
| 625 | fn | `resp` |
| 633 | fn | `req` |
| 641 | fn | `site` |
| 650 | fn | `stored` |
| 662 | fn | `what_is_stored` |
| 718 | fn | `eligibility` |
| 747 | fn | `keys_and_variants` |
| 802 | fn | `storage_round_trip` |

### src/config.rs

1528 baris; 7 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 19 | struct | `Config` |
| 50 | fn | `load` |
| 56 | fn | `parse` |
| 63 | fn | `yaml` |
| 82 | fn | `or_default` |
| 92 | fn | `from_map` |
| 98 | struct | `ServerSection` |
| 131 | fn | `default` |
| 149 | struct | `ListenSection` |
| 157 | struct | `AdminSection` |
| 164 | struct | `Defaults` |
| 184 | enum | `ServerHeader` |
| 192 | fn | `default` |
| 198 | fn | `deserialize` |
| 209 | fn | `default` |
| 225 | struct | `AccessLogSection` |
| 234 | fn | `default` |
| 244 | struct | `RealIpSection` |
| 254 | fn | `default` |
| 265 | struct | `TlsSection` |
| 278 | fn | `default` |
| 289 | const | `LETSENCRYPT` |
| 290 | const | `LETSENCRYPT_STAGING` |
| 296 | struct | `AcmeSection` |
| 314 | fn | `default` |
| 327 | fn | `directory_url` |
| 340 | struct | `CrowdSecSection` |
| 362 | fn | `default` |
| 380 | struct | `AppSecSection` |
| 392 | fn | `default` |
| 404 | enum | `FailureAction` |
| 411 | struct | `CertConfig` |
| 420 | struct | `DefaultSite` |
| 427 | struct | `SiteConfig` |
| 490 | enum | `UpgradeMode` |
| 506 | struct | `FilesConfig` |
| 536 | fn | `default_index` |
| 542 | struct | `SiteCacheConfig` |
| 554 | fn | `deserialize` |
| 557 | struct | `Settings` |
| 565 | struct | `V` |
| 567 | type | `Value` |
| 568 | fn | `expecting` |
| 571 | fn | `visit_bool` |
| 579 | fn | `visit_unit` |
| 582 | fn | `visit_map` |
| 598 | struct | `CacheSection` |
| 609 | fn | `default` |
| 621 | struct | `MtlsConfig` |
| 632 | enum | `UpstreamRef` |
| 638 | fn | `deserialize` |
| 639 | struct | `V` |
| 641 | type | `Value` |
| 642 | fn | `expecting` |
| 645 | fn | `visit_str` |
| 651 | fn | `visit_map` |
| 662 | struct | `PoolConfig` |
| 688 | fn | `single` |
| 702 | fn | `one` |
| 707 | struct | `BackendConfig` |
| 713 | fn | `deserialize` |
| 716 | struct | `Full` |
| 721 | struct | `V` |
| 723 | type | `Value` |
| 724 | fn | `expecting` |
| 727 | fn | `visit_str` |
| 733 | fn | `visit_map` |
| 747 | enum | `Balance` |
| 759 | enum | `ProbeType` |
| 768 | struct | `HealthCheckConfig` |
| 785 | fn | `default` |
| 802 | struct | `PassiveConfig` |
| 810 | fn | `default` |
| 820 | enum | `PluginConfig` |
| 844 | fn | `deserialize` |
| 845 | const | `ONE_KEY` |
| 846 | struct | `V` |
| 848 | type | `Value` |
| 849 | fn | `expecting` |
| 852 | fn | `visit_map` |
| 894 | struct | `HeadersConfig` |
| 903 | struct | `HeaderOps` |
| 911 | struct | `IpFilterConfig` |
| 924 | struct | `CrowdSecPluginConfig` |
| 933 | struct | `RateLimitConfig` |
| 952 | struct | `BasicAuthConfig` |
| 966 | fn | `restricted` |
| 975 | struct | `ForwardAuthConfig` |
| 992 | fn | `five_seconds` |
| 999 | struct | `BodyReplaceConfig` |
| 1012 | struct | `ReplacePair` |
| 1017 | fn | `html_only` |
| 1025 | struct | `BlockExploitsConfig` |
| 1042 | fn | `default` |
| 1057 | enum | `ExploitMode` |
| 1067 | struct | `ExploitException` |
| 1077 | struct | `ConcurrencyConfig` |
| 1087 | fn | `one_second` |
| 1091 | fn | `client_ip` |
| 1095 | fn | `too_many_requests` |
| 1099 | fn | `forbidden` |
| 1103 | fn | `yes` |
| 1109 | struct | `RuleConfig` |
| 1120 | struct | `MatchConfig` |
| 1136 | enum | `StrMatch` |
| 1144 | struct | `StrMatchRaw` |
| 1151 | type | `Error` |
| 1153 | fn | `try_from` |
| 1165 | struct | `HeaderMatch` |
| 1176 | enum | `ActionConfig` |
| 1188 | struct | `ActionRaw` |
| 1197 | type | `Error` |
| 1199 | fn | `try_from` |
| 1229 | struct | `ProxyConfig` |
| 1246 | struct | `RewriteConfig` |
| 1253 | struct | `RedirectConfig` |
| 1260 | fn | `found` |
| 1266 | struct | `RespondConfig` |
| 1276 | struct | `FileConfig` |
| 1284 | mod | `tests` |
| 1288 | fn | `minimal_config` |
| 1310 | fn | `plugins_parse` |
| 1350 | fn | `upstream_forms` |
| 1386 | fn | `example_config_builds` |
| 1464 | fn | `acme_section` |
| 1483 | fn | `typos_are_rejected` |
| 1490 | fn | `yaml_rules` |

### src/crowdsec.rs

1483 baris; 9 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 44 | static | `EPOCH` |
| 46 | fn | `now` |
| 53 | enum | `Remedy` |
| 59 | fn | `parse` |
| 69 | struct | `Decision` |
| 80 | enum | `Slot` |
| 86 | fn | `all` |
| 94 | fn | `add` |
| 118 | fn | `retain` |
| 140 | type | `Map` |
| 143 | trait | `Bits` |
| 144 | const | `WIDTH` |
| 145 | fn | `mask` |
| 149 | const | `WIDTH` |
| 150 | fn | `mask` |
| 159 | const | `WIDTH` |
| 160 | fn | `mask` |
| 170 | struct | `Family` |
| 176 | fn | `default` |
| 185 | fn | `map` |
| 203 | fn | `insert` |
| 213 | fn | `remove` |
| 233 | fn | `check` |
| 253 | fn | `prune` |
| 273 | enum | `Net` |
| 280 | fn | `parse_net` |
| 307 | struct | `Table` |
| 314 | fn | `insert` |
| 322 | fn | `remove` |
| 330 | fn | `check` |
| 337 | fn | `prune` |
| 346 | fn | `go_duration_ms` |
| 388 | struct | `Wire` |
| 402 | trait | `Sink` |
| 403 | fn | `add` |
| 404 | fn | `delete` |
| 408 | fn | `add` |
| 413 | fn | `delete` |
| 418 | struct | `Delta` |
| 421 | fn | `add` |
| 425 | fn | `delete` |
| 430 | struct | `Apply` |
| 439 | fn | `one` |
| 470 | type | `Value` |
| 472 | fn | `deserialize` |
| 478 | type | `Value` |
| 480 | fn | `expecting` |
| 484 | fn | `visit_map` |
| 499 | struct | `List` |
| 502 | type | `Value` |
| 504 | fn | `deserialize` |
| 510 | type | `Value` |
| 512 | fn | `expecting` |
| 516 | fn | `visit_none` |
| 520 | fn | `visit_unit` |
| 524 | fn | `visit_some` |
| 528 | fn | `visit_seq` |
| 538 | struct | `Applied` |
| 544 | fn | `parse_into` |
| 568 | struct | `PullStatus` |
| 575 | struct | `Store` |
| 581 | static | `STORE` |
| 587 | fn | `check` |
| 595 | fn | `len` |
| 602 | fn | `write` |
| 610 | static | `X_API_KEY` |
| 612 | const | `BAN_PAGE` |
| 618 | struct | `Plan` |
| 632 | fn | `fmt` |
| 640 | fn | `query_value` |
| 652 | fn | `base_url` |
| 664 | fn | `build` |
| 751 | fn | `refuse` |
| 769 | const | `LAPI_TIMEOUT` |
| 771 | const | `LAPI_DEADLINE` |
| 773 | const | `LAPI_MAX_RESPONSE` |
| 778 | fn | `pull` |
| 807 | fn | `sync` |
| 831 | const | `PRUNE_EVERY` |
| 833 | struct | `Service` |
| 838 | fn | `new` |
| 845 | fn | `start` |
| 934 | const | `MAX_BODY` |
| 936 | struct | `AppSec` |
| 945 | enum | `Verdict` |
| 952 | fn | `forwarded` |
| 968 | fn | `http_version` |
| 979 | fn | `verdict_status` |
| 981 | struct | `V` |
| 992 | static | `APPSEC_WARNED` |
| 998 | fn | `body_len` |
| 1013 | fn | `inspect` |
| 1106 | struct | `CrowdSecPlugin` |
| 1112 | fn | `new` |
| 1125 | fn | `name` |
| 1129 | fn | `request` |
| 1135 | fn | `appsec` |
| 1141 | mod | `tests` |
| 1144 | fn | `d` |
| 1148 | fn | `ip` |
| 1153 | fn | `go_durations` |
| 1170 | fn | `lookup_cost` |
| 1202 | fn | `slot_stays_small` |
| 1207 | fn | `nets` |
| 1232 | fn | `table_lookups_and_priority` |
| 1289 | const | `STARTUP` |
| 1298 | fn | `stream_full_then_delta` |
| 1346 | fn | `plan` |
| 1352 | fn | `plan_from_config` |
| 1390 | fn | `appsec_request_shape` |
| 1417 | fn | `site_plugin_refuses_held_decisions` |

### src/exploits.rs

2390 baris; 9 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 40 | enum | `Category` |
| 53 | const | `ALL` |
| 65 | fn | `name` |
| 80 | fn | `blocked_by` |
| 94 | fn | `parse` |
| 109 | const | `QUERY` |
| 110 | const | `PATH` |
| 111 | const | `AGENT` |
| 112 | const | `HEADER` |
| 113 | const | `Q` |
| 114 | const | `P` |
| 115 | const | `U` |
| 116 | const | `H` |
| 118 | type | `Verify` |
| 120 | struct | `Def` |
| 134 | fn | `def` |
| 147 | fn | `exact_case` |
| 152 | fn | `verify` |
| 160 | const | `HANDLERS` |
| 277 | const | `SCANNERS` |
| 315 | fn | `defs` |
| 730 | fn | `tautology` |
| 731 | fn | `operand` |
| 740 | fn | `number` |
| 761 | struct | `Rule` |
| 771 | struct | `Engine` |
| 784 | static | `ENGINE` |
| 787 | fn | `new` |
| 867 | fn | `rule_index` |
| 871 | fn | `matches` |
| 891 | fn | `scan` |
| 914 | fn | `scan_slow` |
| 930 | fn | `variants` |
| 949 | fn | `inspect` |
| 979 | fn | `inspect_with` |
| 1038 | fn | `header_suspect` |
| 1044 | fn | `after` |
| 1048 | fn | `contains` |
| 1053 | struct | `Scratch` |
| 1062 | static | `SCRATCH` |
| 1065 | fn | `hex` |
| 1076 | fn | `percent_decode` |
| 1111 | fn | `has_escape` |
| 1120 | fn | `decode_query` |
| 1142 | fn | `strip_comments` |
| 1171 | enum | `Place` |
| 1178 | fn | `fmt` |
| 1188 | struct | `Hit` |
| 1195 | fn | `rule_ids` |
| 1200 | struct | `BlockExploits` |
| 1214 | fn | `categories_mask` |
| 1221 | fn | `new` |
| 1279 | fn | `detect` |
| 1295 | fn | `report` |
| 1316 | fn | `all_rules` |
| 1322 | fn | `may_log` |
| 1323 | static | `WINDOW` |
| 1324 | const | `PER_SECOND` |
| 1345 | fn | `name` |
| 1349 | fn | `needs_path` |
| 1353 | fn | `gate` |
| 1369 | fn | `fuzz_check` |
| 1405 | mod | `tests` |
| 1412 | fn | `detect` |
| 1420 | fn | `detect_with` |
| 1446 | const | `UA` |
| 1450 | type | `Attack` |
| 1456 | const | `ATTACKS` |
| 1961 | const | `ORDINARY` |
| 2098 | fn | `attacks_are_caught_by_their_rule` |
| 2110 | fn | `ordinary_requests_pass` |
| 2122 | fn | `needles_never_hide_a_match` |
| 2141 | fn | `every_rule_has_an_attack` |
| 2151 | fn | `decoding` |
| 2185 | fn | `tautologies` |
| 2216 | fn | `plugin` |
| 2228 | fn | `settings` |
| 2275 | fn | `block_and_log_modes` |
| 2318 | fn | `cost` |

### src/files.rs

1271 baris; 5 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 34 | const | `CHUNK` |
| 36 | const | `LIST_LIMIT` |
| 39 | enum | `Encoding` |
| 46 | fn | `parse` |
| 57 | fn | `suffix` |
| 65 | fn | `token` |
| 73 | fn | `bit` |
| 84 | struct | `Accepted` |
| 87 | fn | `from_request` |
| 124 | fn | `accepts` |
| 129 | fn | `bits` |
| 135 | fn | `accepts_coding` |
| 143 | enum | `Part` |
| 150 | struct | `Candidate` |
| 158 | fn | `parse` |
| 183 | fn | `expand` |
| 195 | fn | `wants_dir` |
| 201 | enum | `Hide` |
| 207 | fn | `parse` |
| 225 | fn | `hides` |
| 235 | struct | `Files` |
| 249 | fn | `compile` |
| 307 | fn | `takes` |
| 311 | fn | `hidden` |
| 320 | fn | `find` |
| 415 | fn | `file` |
| 437 | fn | `lookup` |
| 450 | struct | `Lookup` |
| 462 | fn | `with_query` |
| 478 | fn | `show` |
| 482 | enum | `Found` |
| 493 | fn | `into_local` |
| 513 | struct | `FileReply` |
| 526 | fn | `reply` |
| 568 | fn | `read_exact_at` |
| 588 | static | `NO_OPENAT2` |
| 591 | fn | `open_beneath` |
| 642 | fn | `content_type` |
| 712 | fn | `etag_listed` |
| 731 | fn | `http_date` |
| 735 | fn | `secs` |
| 740 | enum | `Plan` |
| 750 | fn | `plan` |
| 797 | fn | `parse_range` |
| 835 | fn | `send` |
| 918 | fn | `html_escape` |
| 931 | fn | `url_escape` |
| 941 | fn | `human_size` |
| 942 | const | `UNITS` |
| 957 | fn | `listing` |
| 1009 | mod | `tests` |
| 1013 | struct | `Dir` |
| 1015 | fn | `drop` |
| 1020 | fn | `tree` |
| 1044 | fn | `files` |
| 1050 | fn | `lookup` |
| 1064 | fn | `body` |
| 1075 | fn | `serving_and_confinement` |
| 1107 | fn | `precompressed_try_files_browse_prefix` |
| 1151 | fn | `settings_are_checked` |
| 1172 | fn | `reply_for` |
| 1187 | fn | `plan_with` |
| 1196 | fn | `conditionals_and_ranges` |
| 1265 | fn | `content_types` |

### src/fuzzing.rs

493 baris; 0 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 22 | fn | `config` |
| 39 | fn | `ip_backends_only` |
| 71 | fn | `path` |
| 88 | fn | `route` |
| 91 | static | `ROUTES` |
| 158 | fn | `block_exploits` |
| 167 | fn | `body_replace` |
| 218 | fn | `access_log` |
| 285 | const | `ROUTES` |
| 365 | static | `ROUTER` |
| 377 | fn | `request` |

### src/httpc.rs

202 baris; 0 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 16 | struct | `Reply` |
| 22 | struct | `Client` |
| 29 | const | `IDLE` |
| 34 | fn | `new` |
| 44 | fn | `resolve` |
| 67 | fn | `send` |
| 108 | fn | `exchange` |

### src/lib.rs

34 baris; 0 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 7 | mod | `accesslog` |
| 8 | mod | `acme` |
| 9 | mod | `admin` |
| 10 | mod | `auth` |
| 11 | mod | `cache` |
| 12 | mod | `config` |
| 13 | mod | `crowdsec` |
| 14 | mod | `exploits` |
| 15 | mod | `files` |
| 17 | mod | `fuzzing` |
| 18 | mod | `httpc` |
| 19 | mod | `limits` |
| 20 | mod | `logger` |
| 21 | mod | `metrics` |
| 22 | mod | `pathnorm` |
| 23 | mod | `plugin` |
| 24 | mod | `proxy` |
| 25 | mod | `realip` |
| 26 | mod | `reload` |
| 27 | mod | `replace` |
| 28 | mod | `route` |
| 29 | mod | `rules` |
| 30 | mod | `runtime` |
| 31 | mod | `timefmt` |
| 32 | mod | `tls` |
| 33 | mod | `units` |
| 34 | mod | `upstream` |

### src/limits.rs

468 baris; 4 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 26 | type | `Hasher` |
| 30 | enum | `Key` |
| 37 | fn | `parse` |
| 52 | fn | `of` |
| 66 | fn | `client` |
| 73 | fn | `check_status` |
| 82 | static | `CARRIED` |
| 85 | fn | `carried` |
| 101 | struct | `Window` |
| 108 | struct | `RateLimit` |
| 119 | fn | `new` |
| 145 | fn | `name` |
| 149 | fn | `needs_path` |
| 153 | fn | `request` |
| 175 | struct | `InFlight` |
| 181 | struct | `Held` |
| 187 | fn | `drop` |
| 194 | struct | `Concurrency` |
| 204 | fn | `new` |
| 227 | fn | `name` |
| 231 | fn | `needs_path` |
| 235 | fn | `admit` |
| 256 | mod | `tests` |
| 262 | fn | `chain` |
| 277 | fn | `view` |
| 296 | fn | `rate_limit_per_client_and_match` |
| 372 | fn | `rate_limit_by_header` |
| 399 | fn | `concurrency_holds_until_dropped` |
| 443 | fn | `bad_settings` |

### src/logger.rs

128 baris; 1 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 14 | fn | `benign` |
| 23 | fn | `chatty` |
| 31 | struct | `Logger` |
| 36 | fn | `enabled` |
| 40 | fn | `log` |
| 65 | fn | `flush` |
| 69 | fn | `init` |
| 81 | mod | `tests` |
| 85 | fn | `only_the_pool_race_is_benign` |

### src/main.rs

297 baris; 0 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 36 | static | `GLOBAL` |
| 40 | fn | `socket_options` |
| 52 | fn | `listener` |
| 58 | const | `DEFAULT_CONFIG` |
| 62 | struct | `Cli` |
| 68 | enum | `Cmd` |
| 90 | fn | `huge_pages` |
| 109 | fn | `main` |
| 130 | fn | `check` |
| 176 | fn | `run` |

### src/metrics.rs

342 baris; 1 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 21 | const | `LATENCY_BUCKETS` |
| 178 | const | `CLASSES` |
| 181 | struct | `SiteMetrics` |
| 191 | fn | `new` |
| 202 | fn | `cache` |
| 208 | fn | `response` |
| 218 | fn | `upstream_header` |
| 224 | fn | `forget_site` |
| 250 | fn | `init` |
| 273 | static | `PROCESS` |
| 276 | fn | `render` |
| 324 | mod | `tests` |
| 328 | fn | `status_classes` |

### src/pathnorm.rs

118 baris; 2 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 7 | enum | `PathError` |
| 12 | fn | `hex` |
| 21 | fn | `percent_decode` |
| 51 | fn | `normalize` |
| 84 | mod | `tests` |
| 87 | fn | `n` |
| 92 | fn | `resolves_like_nginx` |
| 111 | fn | `rejects_bad_input` |

### src/plugin.rs

573 baris; 2 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 29 | struct | `Local` |
| 38 | fn | `status` |
| 47 | fn | `with_body` |
| 56 | fn | `redirect` |
| 68 | enum | `Gate` |
| 80 | type | `UpstreamFor` |
| 84 | struct | `Effects` |
| 94 | enum | `Step` |
| 103 | struct | `CheckRequest` |
| 112 | type | `UpstreamEdits` |
| 115 | trait | `Plugin` |
| 116 | fn | `name` |
| 119 | fn | `request` |
| 125 | fn | `admit` |
| 130 | fn | `gate` |
| 139 | fn | `check` |
| 144 | fn | `upstream_request` |
| 149 | fn | `response` |
| 154 | fn | `needs_path` |
| 159 | fn | `appsec` |
| 165 | struct | `Chain` |
| 179 | fn | `build` |
| 259 | fn | `pools` |
| 264 | fn | `uses_site_pool` |
| 268 | fn | `needs_path` |
| 272 | fn | `appsec` |
| 276 | fn | `names` |
| 282 | fn | `run` |
| 298 | fn | `check` |
| 308 | fn | `request` |
| 322 | fn | `upstream_request` |
| 326 | fn | `response` |
| 333 | fn | `no_upstreams` |
| 338 | struct | `RulesPlugin` |
| 341 | fn | `name` |
| 345 | fn | `needs_path` |
| 349 | fn | `gate` |
| 369 | struct | `Ops` |
| 375 | fn | `compile` |
| 391 | struct | `HeadersPlugin` |
| 397 | fn | `new` |
| 406 | fn | `name` |
| 410 | fn | `upstream_request` |
| 420 | fn | `response` |
| 432 | struct | `IpFilter` |
| 439 | fn | `new` |
| 457 | fn | `name` |
| 461 | fn | `request` |
| 474 | mod | `tests` |
| 479 | fn | `chain` |
| 494 | fn | `view` |
| 508 | fn | `ip_filter` |
| 543 | fn | `order_and_headers` |

### src/proxy.rs

2033 baris; 14 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 40 | struct | `ReloadStatus` |
| 47 | struct | `Shared` |
| 66 | struct | `Conduit` |
| 71 | fn | `new` |
| 83 | fn | `worker_pool_group` |
| 84 | static | `NEXT` |
| 86 | static | `GROUP` |
| 91 | struct | `Ctx` |
| 130 | struct | `UpgradeLog` |
| 142 | fn | `new` |
| 173 | fn | `drop` |
| 207 | static | `X_REQUEST_ID` |
| 208 | static | `CACHE_STATUS` |
| 212 | static | `X_FORWARDED_FOR` |
| 214 | static | `X_REAL_IP` |
| 215 | static | `X_FORWARDED_PROTO` |
| 217 | static | `X_FORWARDED_HOST` |
| 220 | static | `X_REQUEST_ID_SENT` |
| 224 | fn | `new_request_id` |
| 225 | static | `PREFIX` |
| 233 | const | `BLOCK` |
| 234 | static | `NEXT` |
| 236 | static | `RANGE` |
| 238 | const | `HEX` |
| 264 | fn | `ip_text` |
| 282 | fn | `incoming_request_id` |
| 291 | fn | `request_host` |
| 320 | fn | `request_uri` |
| 331 | fn | `header_str` |
| 338 | fn | `forwards_upgrade` |
| 351 | fn | `has_body` |
| 359 | const | `ACME_PREFIX` |
| 361 | fn | `acme_response` |
| 382 | fn | `client_scheme` |
| 396 | struct | `Conn` |
| 405 | fn | `client_cert` |
| 415 | fn | `check_client_cert` |
| 443 | fn | `refused` |
| 449 | static | `X_CLIENT_VERIFY` |
| 450 | static | `X_CLIENT_CERT_SUBJECT` |
| 451 | static | `X_CLIENT_CERT_FINGERPRINT` |
| 455 | fn | `client_cert_headers` |
| 478 | fn | `pool_of` |
| 486 | fn | `split_target` |
| 494 | fn | `routed_target` |
| 511 | fn | `decide` |
| 583 | fn | `run_plugins` |
| 627 | fn | `appsec` |
| 672 | fn | `enable_compression` |
| 686 | enum | `Next` |
| 694 | fn | `files_and_cache` |
| 745 | fn | `status_page` |
| 758 | fn | `decorate` |
| 781 | fn | `respond` |
| 813 | fn | `compressible` |
| 822 | const | `TYPES` |
| 835 | fn | `proto` |
| 846 | fn | `final_status` |
| 860 | fn | `upstream_error_status` |
| 870 | type | `CTX` |
| 872 | fn | `new_ctx` |
| 876 | fn | `request_filter` |
| 977 | fn | `request_body_filter` |
| 1006 | fn | `upstream_peer` |
| 1060 | fn | `fail_to_connect` |
| 1080 | fn | `connected_to_upstream` |
| 1094 | fn | `upstream_request_filter` |
| 1207 | fn | `upstream_response_filter` |
| 1248 | fn | `response_filter` |
| 1291 | fn | `response_body_filter` |
| 1312 | fn | `range_header_filter` |
| 1324 | fn | `request_cache_filter` |
| 1343 | fn | `cache_key_callback` |
| 1390 | fn | `response_cache_filter` |
| 1402 | fn | `cache_vary_filter` |
| 1414 | fn | `should_serve_stale` |
| 1424 | fn | `fail_to_proxy` |
| 1469 | fn | `suppress_error_log` |
| 1474 | fn | `logging` |
| 1559 | mod | `tests` |
| 1565 | fn | `each_worker_thread_has_its_own_pool_group` |
| 1574 | fn | `req` |
| 1582 | fn | `runtime` |
| 1586 | const | `BASE` |
| 1608 | fn | `answer` |
| 1616 | fn | `run_with` |
| 1633 | fn | `run` |
| 1638 | fn | `host_parsing` |
| 1653 | fn | `target_without_path_is_400` |
| 1667 | fn | `https_redirect_runs_before_plugins` |
| 1693 | fn | `plugins_see_normalized_path_and_client` |
| 1707 | fn | `routed_requests_skip_files_and_change_target` |
| 1759 | fn | `body_limit_and_default_site` |
| 1772 | fn | `acme_challenge` |
| 1799 | fn | `acme_challenge_from_memory` |
| 1830 | fn | `request_ids` |
| 1860 | fn | `upstream_timeouts_are_504` |
| 1880 | fn | `ip_text_matches_display` |
| 1897 | fn | `compressible_types` |
| 1910 | fn | `client_certificates` |

### src/realip.rs

205 baris; 4 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 9 | const | `CLOUDFLARE` |
| 35 | struct | `Cidr` |
| 41 | fn | `parse` |
| 62 | fn | `contains` |
| 82 | fn | `canonical` |
| 90 | struct | `RealIp` |
| 97 | fn | `new` |
| 117 | fn | `is_trusted` |
| 122 | fn | `resolve` |
| 150 | mod | `tests` |
| 153 | fn | `ip` |
| 158 | fn | `cidr_contains` |
| 172 | fn | `untrusted_peer_header_ignored` |
| 182 | fn | `trusted_peer_header_used` |
| 196 | fn | `recursive_skips_trusted_hops` |

### src/reload.rs

219 baris; 0 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 23 | type | `Stamp` |
| 25 | fn | `stamp` |
| 36 | fn | `snapshot` |
| 40 | struct | `Reloader` |
| 47 | fn | `new` |
| 56 | fn | `changed` |
| 64 | fn | `reload` |
| 169 | fn | `record` |
| 186 | fn | `start` |

### src/replace.rs

324 baris; 5 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 22 | struct | `BodyReplace` |
| 33 | fn | `new` |
| 69 | fn | `takes` |
| 84 | fn | `rewritable` |
| 101 | fn | `rewritten_headers` |
| 121 | struct | `Stream` |
| 128 | fn | `new` |
| 136 | fn | `feed` |
| 165 | struct | `BodyReplacePlugin` |
| 168 | fn | `new` |
| 174 | fn | `name` |
| 178 | fn | `needs_path` |
| 182 | fn | `gate` |
| 191 | mod | `tests` |
| 195 | fn | `replacer` |
| 212 | fn | `whole` |
| 217 | fn | `replaces_every_occurrence_first_pattern_wins` |
| 229 | fn | `chunk_boundaries_do_not_matter` |
| 257 | fn | `holds_back_no_more_than_needed` |
| 266 | fn | `which_responses` |
| 305 | fn | `bad_configs` |

### src/route.rs

259 baris; 4 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 18 | enum | `Rewrite` |
| 24 | struct | `Route` |
| 33 | fn | `fmt` |
| 42 | fn | `path_prefix` |
| 53 | fn | `compile` |
| 97 | fn | `rewrites` |
| 102 | fn | `path` |
| 125 | fn | `target` |
| 139 | fn | `encode_path` |
| 140 | const | `HEX` |
| 172 | mod | `tests` |
| 176 | fn | `route` |
| 189 | fn | `target` |
| 194 | fn | `strip_and_add_prefix` |
| 212 | fn | `regex_rewrite` |
| 226 | fn | `decoded_bytes_are_encoded_again` |
| 236 | fn | `bad_configs` |

### src/rules.rs

481 baris; 4 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 16 | type | `Resolve` |
| 19 | enum | `BytesMatch` |
| 26 | fn | `compile` |
| 36 | fn | `is_match` |
| 46 | enum | `HeaderCond` |
| 53 | fn | `compile` |
| 73 | fn | `is_match` |
| 83 | struct | `RequestView` |
| 100 | struct | `Matcher` |
| 110 | fn | `compile` |
| 156 | fn | `is_match` |
| 188 | enum | `Part` |
| 198 | struct | `Template` |
| 201 | fn | `parse` |
| 227 | fn | `render` |
| 242 | type | `Headers` |
| 244 | fn | `compile_headers` |
| 259 | enum | `Action` |
| 274 | fn | `status_ok` |
| 283 | fn | `compile` |
| 331 | struct | `Rule` |
| 338 | fn | `compile` |
| 358 | fn | `is_match` |
| 364 | mod | `tests` |
| 368 | fn | `no_pools` |
| 372 | fn | `site_rules` |
| 379 | fn | `rules` |
| 389 | fn | `view` |
| 407 | const | `SITE` |
| 410 | fn | `forgejo_browser_redirect` |
| 430 | fn | `header_presence` |
| 456 | fn | `path_regex_on_normalized_path` |
| 472 | fn | `bad_rules_are_rejected` |

### src/runtime.rs

874 baris; 7 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 27 | struct | `Site` |
| 50 | struct | `Mtls` |
| 56 | struct | `AcmeSite` |
| 65 | struct | `AcmePlan` |
| 79 | fn | `fnv1a` |
| 87 | fn | `acme_dir` |
| 106 | fn | `acme_plan` |
| 127 | fn | `acme_site` |
| 190 | struct | `Router` |
| 199 | fn | `lookup` |
| 208 | fn | `len` |
| 213 | struct | `Runtime` |
| 240 | struct | `Built` |
| 246 | fn | `valid_id` |
| 252 | fn | `normalize_host` |
| 265 | fn | `compile_site` |
| 371 | fn | `build` |
| 551 | fn | `server_header` |
| 563 | mod | `tests` |
| 568 | fn | `server_header_setting` |
| 599 | fn | `pools_and_references` |
| 665 | fn | `proxy_rule_pools` |
| 741 | fn | `routing` |
| 770 | fn | `acme_sites` |
| 842 | fn | `acme_storage_per_directory` |
| 861 | fn | `duplicate_hosts_rejected` |

### src/timefmt.rs

94 baris; 1 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 7 | fn | `civil_from_days` |
| 20 | fn | `seconds_part` |
| 32 | fn | `rfc3339` |
| 38 | fn | `now_secs` |
| 45 | static | `CACHE` |
| 50 | fn | `write_rfc3339_millis` |
| 70 | fn | `rfc3339_millis` |
| 77 | mod | `tests` |
| 82 | fn | `known_instants` |

### src/tls.rs

627 baris; 5 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 30 | struct | `CertKey` |
| 42 | fn | `load` |
| 95 | struct | `CertStore` |
| 106 | fn | `build` |
| 139 | fn | `add_override` |
| 151 | fn | `add_managed` |
| 169 | fn | `covers` |
| 173 | fn | `select_specific` |
| 183 | fn | `select` |
| 191 | fn | `all` |
| 195 | fn | `summary` |
| 214 | struct | `CertResolver` |
| 219 | fn | `new` |
| 225 | struct | `ClientTrust` |
| 234 | fn | `load` |
| 255 | struct | `ClientCert` |
| 261 | enum | `ClientStatus` |
| 274 | fn | `read` |
| 306 | fn | `distinguished_name` |
| 348 | fn | `sni_from_extension` |
| 364 | static | `TRUST_INDEX` |
| 368 | fn | `SSL_set_session_id_context` |
| 375 | fn | `client_hello_sni` |
| 392 | fn | `ask_for_certificate` |
| 417 | fn | `ask_client_certs` |
| 437 | fn | `handshake_complete_callback` |
| 446 | fn | `certificate_callback` |
| 470 | mod | `test_certs` |
| 482 | fn | `self_signed` |
| 516 | mod | `tests` |
| 519 | fn | `write_pair` |
| 533 | fn | `selection_order` |
| 557 | fn | `mismatched_key_rejected` |
| 573 | fn | `sni_extension` |
| 597 | fn | `subject_as_rfc4514` |
| 612 | fn | `client_trust_from_file` |

### src/units.rs

121 baris; 2 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 10 | enum | `Raw` |
| 18 | struct | `Dur` |
| 21 | type | `Error` |
| 23 | fn | `try_from` |
| 31 | fn | `parse_duration` |
| 52 | struct | `Size` |
| 55 | fn | `fmt` |
| 64 | type | `Error` |
| 66 | fn | `try_from` |
| 75 | fn | `parse_size` |
| 94 | mod | `tests` |
| 98 | fn | `durations` |
| 112 | fn | `sizes` |

### src/upstream.rs

838 baris; 6 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 35 | const | `MAX_ITERATIONS` |
| 37 | static | `EPOCH` |
| 40 | fn | `now_ms` |
| 45 | struct | `Target` |
| 64 | fn | `ejected` |
| 70 | struct | `Lease` |
| 73 | fn | `new` |
| 79 | fn | `target` |
| 85 | fn | `drop` |
| 92 | enum | `HashKey` |
| 102 | fn | `parse` |
| 131 | trait | `Select` |
| 132 | fn | `pick` |
| 133 | fn | `backends` |
| 141 | fn | `pick` |
| 145 | fn | `backends` |
| 150 | struct | `Member` |
| 156 | struct | `MemberStatus` |
| 164 | struct | `Pool` |
| 185 | struct | `Resolved` |
| 193 | fn | `resolve` |
| 264 | fn | `build` |
| 346 | fn | `len` |
| 350 | fn | `member` |
| 359 | fn | `key` |
| 390 | fn | `select` |
| 405 | fn | `choose` |
| 422 | fn | `least_conn` |
| 440 | fn | `note_retry` |
| 445 | fn | `report_failure` |
| 463 | fn | `report_success` |
| 469 | fn | `status` |
| 481 | fn | `probe_if_due` |
| 499 | fn | `load_balancer` |
| 516 | static | `PROBE_HTTP` |
| 517 | static | `PROBE_TCP` |
| 521 | struct | `Probe` |
| 535 | fn | `new` |
| 549 | fn | `target` |
| 557 | fn | `run` |
| 596 | fn | `check` |
| 606 | fn | `health_status_change` |
| 620 | fn | `health_threshold` |
| 626 | struct | `HealthService` |
| 631 | fn | `new` |
| 638 | fn | `start` |
| 655 | mod | `tests` |
| 659 | fn | `pool_cfg` |
| 672 | fn | `pick` |
| 677 | fn | `urls` |
| 719 | fn | `round_robin_spreads_and_skips_excluded` |
| 751 | fn | `passive_ejection_and_panic_mode` |
| 781 | fn | `least_conn_prefers_idle_backends` |
| 804 | fn | `hashing_is_sticky` |
| 828 | fn | `unchanged_pool_is_reused` |

### tests/e2e.rs

5160 baris; 20 deklarasi test ditemukan.

| Baris | Jenis | Simbol |
|---:|---|---|
| 31 | fn | `free_port` |
| 32 | static | `NEXT` |
| 42 | fn | `self_signed` |
| 81 | fn | `echo_upstream` |
| 95 | fn | `serve_echo` |
| 135 | struct | `Resp` |
| 142 | fn | `header` |
| 150 | fn | `parse` |
| 170 | fn | `request` |
| 179 | fn | `read_all` |
| 190 | fn | `http` |
| 198 | fn | `tls_connect` |
| 224 | fn | `https` |
| 231 | fn | `metric` |
| 239 | fn | `status_json` |
| 246 | fn | `backend_state` |
| 264 | struct | `Proxy` |
| 270 | fn | `drop` |
| 279 | fn | `test_dir` |
| 285 | fn | `spawn` |
| 297 | fn | `signal` |
| 306 | fn | `wait_for_port` |
| 317 | fn | `wait_until` |
| 325 | struct | `Ports` |
| 340 | fn | `new` |
| 354 | fn | `config` |
| 435 | fn | `end_to_end` |
| 780 | fn | `panic_text` |
| 788 | fn | `tally` |
| 800 | fn | `graceful_upgrade` |
| 874 | fn | `load_until` |
| 904 | fn | `presented` |
| 909 | fn | `rotation_and_reload_under_load` |
| 1011 | fn | `wait_exit` |
| 1028 | struct | `Pebble` |
| 1032 | const | `PEBBLE_TAG` |
| 1034 | fn | `docker` |
| 1045 | fn | `start` |
| 1106 | fn | `drop` |
| 1114 | fn | `served` |
| 1129 | fn | `expiring` |
| 1166 | fn | `mode` |
| 1177 | fn | `acme_with_pebble` |
| 1356 | struct | `CrowdSec` |
| 1362 | const | `CROWDSEC_TAG` |
| 1363 | const | `BOUNCER_KEY` |
| 1366 | fn | `exchange` |
| 1375 | fn | `start` |
| 1474 | fn | `cscli` |
| 1482 | fn | `drop` |
| 1503 | fn | `crowdsec_bouncer_and_appsec` |
| 1740 | fn | `slow_upstream` |
| 1774 | fn | `rate_and_concurrency_limits` |
| 1908 | struct | `Ca` |
| 1913 | fn | `ec_key` |
| 1918 | fn | `cert_builder` |
| 1944 | fn | `new` |
| 1974 | fn | `issue` |
| 1985 | struct | `MtlsReply` |
| 1993 | fn | `mtls_get` |
| 2041 | fn | `read_response` |
| 2062 | fn | `mutual_tls` |
| 2321 | fn | `auth_service` |
| 2368 | fn | `auth_plugins` |
| 2560 | fn | `raw_http` |
| 2568 | fn | `file_server` |
| 2804 | type | `Counts` |
| 2806 | fn | `cache_upstream` |
| 2821 | fn | `serve_cacheable` |
| 2913 | fn | `response_cache` |
| 3181 | type | `Heads` |
| 3185 | fn | `read_head` |
| 3197 | fn | `head_value` |
| 3209 | fn | `upgrade_upstream` |
| 3252 | fn | `open_upgrade` |
| 3260 | fn | `websocket_request` |
| 3268 | fn | `tailscale_request` |
| 3276 | fn | `echo` |
| 3284 | fn | `idle_close` |
| 3293 | fn | `protocol_upgrades` |
| 3461 | fn | `noise` |
| 3473 | fn | `hex` |
| 3480 | fn | `body_upstream` |
| 3481 | fn | `read_n` |
| 3494 | fn | `serve` |
| 3570 | fn | `body_exchange` |
| 3625 | fn | `request_and_response_bodies` |
| 3718 | const | `UP` |
| 3721 | const | `DOWN` |
| 3723 | const | `HANG` |
| 3728 | struct | `Flappy` |
| 3736 | fn | `start` |
| 3790 | fn | `set` |
| 3804 | fn | `serve_flappy` |
| 3851 | struct | `Rec` |
| 3860 | struct | `Answer` |
| 3869 | fn | `keepalive_exchange` |
| 3936 | fn | `flap_client` |
| 4009 | fn | `open_fds` |
| 4018 | fn | `upstream_flapping` |
| 4262 | fn | `upgrade_with_open_websockets` |
| 4377 | fn | `idle_and_malformed_clients` |
| 4471 | fn | `slow_request_head_is_cut_off` |
| 4516 | fn | `cpu_time` |
| 4532 | fn | `file_descriptors_run_out` |
| 4607 | fn | `dechunk` |
| 4624 | fn | `html_upstream` |
| 4695 | fn | `routes_and_body_replace` |
| 4762 | type | `Case` |
| 4838 | fn | `recording_upstream` |
| 4874 | fn | `block_exploits` |
| 4940 | type | `Attack` |
| 5035 | fn | `raw_request` |
| 5043 | fn | `server_header` |

