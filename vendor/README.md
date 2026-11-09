# Pingora fork

`vendor/pingora` adalah Pingora 0.9.1 (tag `0.9.1`, commit `f86ffda`) dengan patch performa
conduit di branch `conduit`. Setiap patch adalah satu commit di atas tag itu, lengkap dengan
alasan dan test-nya, jadi bisa dibaca dengan `git -C vendor/pingora log -p 0.9.1..conduit`.

Fork ini ada karena selisih throughput HTTP/1.1 dan HTTPS dengan nginx ada di dalam Pingora,
bukan di kode conduit (lihat "Throughput HTTP/1.1 dan HTTPS" di
[dokumentasi teknis](../docs/teknis.md#throughput-http11-dan-https)). Keputusan 2026-11-07: tetap di
Pingora dan menambal titik panasnya, bukan menulis stack HTTP/1 sendiri.

## Cara terpasang

- `vendor/pingora` adalah git submodule yang menunjuk ke branch `conduit` di repo fork. Clone
  conduit dengan `git clone --recurse-submodules`, atau jalankan
  `git submodule update --init` setelah clone biasa. Patch baru di-commit di fork, di-push ke
  branch `conduit`, lalu commit submodule di repo conduit diperbarui.
- `Cargo.toml` memasang semua crate `pingora-*` dari folder ini lewat `[patch.crates-io]`,
  supaya tidak ada crate Pingora yang dibangun dua kali. Versinya tetap 0.9.1, dan
  `Cargo.lock` menyimpan versi dependensi yang sama persis dengan sebelum fork.
- `Dockerfile` menyalin `vendor/` ke build release. `.dockerignore` membuang `.git` dan
  `target` di dalamnya.
- `.graphifyignore` mengecualikan `vendor/pingora/` dari graph. File ini tetap masuk.

## Patch

Angka diukur dengan `scripts/ab.sh` (binary lama dan baru bergantian di CPU yang sama,
7 putaran, median), build release biasa tanpa PGO, keep-alive 64 koneksi, body 2 KB, access log
aktif. "CPU" adalah waktu CPU conduit per request.

| Commit | Isi | CPU HTTP | CPU HTTPS |
|---|---|---:|---:|
| `79bf391` | Koneksi upstream yang idle disimpan tanpa task per koneksi | −9,3% | −7,4% |
| `32d98ca` | Cek reuse membandingkan alamat dari digest, tanpa `getpeername()` | ±0 (1 syscall/request lebih sedikit) | ±0 |
| `ecef83b` | Cek tipe MIME untuk kompresi dengan prefix, tanpa regex | | |
| `9586c10` | Pembuangan header hop-by-hop dilewati kalau tidak ada | | |
| `c448eff` | `insert_header` dengan nama yang sudah di-parse tidak mem-parse ulang (`CasedHeaderName`) | | |
| tiga di atas, bersama | | −3,9% | −3,0% |
| `9c64421` | Huruf besar-kecil nama header disimpan sebagai gaya + pengecualian, bukan HeaderMap kedua | ±0 | −1,0% |
| `3357c9e` | Satu batch task respons di-flush sekali, bukan sekali per task | ±0 (sudah 1 send) | ±0 |
| `bb3859d` | Header respons upstream diserahkan ke proxy, bukan disalin | −1,4% | −2,2% |
| `c4cce19` | Channel task HTTP/1 sendiri (`task_pipe`), bukan dua `tokio::sync::mpsc` per request | ±0 | −2,3% |
| `0a6aee4` | Module HTTP di context request dicari berurutan, tanpa indeks `Arc<HashMap>` bersama | ±0¹ | −2,7%¹ |

¹ Diukur bersama perubahan kecil di conduit (tahap A, lihat [dokumentasi teknis](../docs/teknis.md)): gzip −1,9%,
HTTP/2 −2,9%.

Total sampai `3357c9e`: CPU HTTP −14%, HTTPS −12%. Resolusi CPU `scripts/ab.sh` sekitar ±1%
(tick 10 ms atas 50.000 request), jadi patch di bawah 1% lebih jelas terlihat di profil samply:
`bb3859d` menghapus clone header respons (1,05% sampel worker → 0), `c4cce19` memotong porsi
channel dari 3,1% ke 1,5%.

`3357c9e` menyasar respons yang dikompresi conduit (skenario `gzip` di `scripts/ab.sh`, HTTP
dengan `Accept-Encoding: gzip`): req/s +22,8%, CPU per request −13,5%, p99 −19,5%.

`e539402` hanya membungkam satu warning compiler di crate `pingora`.

### Patch stabilitas

| Commit | Isi |
|---|---|
| `7ffe1c4` | Seluruh header request HTTP/1 dibatasi `read_timeout`, bukan tiap read |
| `499f89d` | Header `Server` di respons buatan Pingora sendiri bisa diatur (`set_server_header`) |

`7ffe1c4`: Pingora menerapkan `read_timeout` (60 detik) ke setiap read selama header request
masuk, jadi klien yang mengirim satu byte tiap beberapa detik (slowloris) bisa menahan koneksi
dan buffernya selamanya. Sekarang, begitu byte header pertama datang (termasuk sisa pipelining),
sisa header harus lengkap sebelum `now + read_timeout`; kalau tidak, read gagal dengan
`ReadTimedout` ("request header incomplete after ..."). Menunggu request berikutnya di koneksi
keep-alive (sebelum byte pertama) tidak berubah; batas itu diatur conduit lewat
`defaults.keepalive_timeout`. Test: `read_timeout_bounds_the_whole_request_header` di
`pingora-core`, dan e2e conduit `slow_request_head_is_cut_off` (koneksi diputus setelah 62 detik).

`499f89d`: respons yang ditulis Pingora sendiri, bukan oleh aplikasi (400 untuk header request
yang tidak bisa di-parse, 405 untuk `CONNECT`, 502 saat `proxy_upstream_filter` menolak, jawaban
purge), selalu membawa `Server: Pingora`. `pingora_core::protocols::http::set_server_header()`
mengatur nilainya untuk seluruh proses, atau membuang header itu dengan `None`; default tetap
`Pingora`. Respons itu kini dibangun setiap kali (jalur error, jarang) alih-alih disalin dari
static yang dibuat sekali, supaya perubahan saat reload langsung berlaku. conduit memanggilnya saat
start dan setiap reload dengan `defaults.server_header`. Test: `server_header_follows_the_setting`
di `pingora-core`, dan e2e conduit `server_header`.

### `79bf391`: pool koneksi upstream tanpa task

Di Pingora asli, setiap kali koneksi upstream dikembalikan ke pool, Pingora membuat satu task
tokio yang memegang koneksi itu di balik `Arc<Mutex>` dan menunggu oneshot, `Notify`, dan timer.
Request berikutnya harus membangunkan task itu dan menunggu task itu selesai sebelum bisa
memakai koneksinya. Untuk upstream keep-alive yang sibuk, itu berarti satu spawn task, satu
timer, beberapa alokasi, dan dua kali wake task untuk setiap request.

Sekarang `TransportConnector` menyimpan koneksi idle di pool sendiri
(`pingora-core/src/connectors/idle.rs`):

- Per kunci reuse, koneksi yang paling baru dikembalikan dipakai lebih dulu. Batas ukuran
  global (`upstream_keepalive_pool_size`) dan pembuangan koneksi paling lama tetap sama.
- Koneksi idle tidak dipoll siapa pun. Koneksi dicek saat diambil, dan satu task penyapu per
  pool mengecek semuanya setiap detik: koneksi yang ditutup server, yang berisi data tak
  terduga, atau yang lewat idle timeout ditutup.
- Kedua cek melihat FIN, RST, atau data yang datang selama idle, karena koneksi tetap terdaftar
  di I/O driver tokio, yang terus mencatat readiness-nya.

Bedanya dengan aslinya:

- Koneksi yang ditutup server tetap terbuka di sisi conduit sampai sapuan berikutnya
  (paling lama ±1 detik) atau sampai diambil.
- Idle timeout bisa telat sampai 1 detik. Koneksi yang sudah lewat timeout tidak pernah
  diberikan ke request.
- `pingora_pool::ConnectionPool` tidak diubah, dan masih dipakai untuk HTTP/2.

### Patch kecil

- `32d98ca`: Pingora memanggil `getpeername()` setiap kali koneksi dipakai ulang, untuk
  memastikan koneksinya memang ke peer yang benar. Connector TCP sudah mencatat alamat itu di
  digest koneksi saat connect, jadi `HttpPeer` sekarang membandingkan dengan catatan itu. Peer
  lewat proxy dan peer yang meng-override `matches_fd()` tetap memakai cara lama.
- `ecef83b`: saat kompresi aktif, setiap respons dicek tipe MIME-nya, termasuk saat klien tidak
  mengirim `Accept-Encoding`. Regex diganti `starts_with` pada daftar yang sama. Ada test yang
  mengunci hasil regex lama, termasuk sensitivitas huruf besar-kecil.
- `9586c10`: setiap request ke upstream HTTP/1 menjalankan sepuluh lookup header hop-by-hop
  yang hampir selalu tidak menemukan apa-apa. Sekarang nama header yang ada dicek sekali jalan
  dulu.
- `c448eff`: `append_header` dan `insert_header` mengubah nama jadi bentuk tulisannya, lalu
  mem-parse balik, termasuk untuk konstanta `HeaderName`. Nama string dengan huruf besar
  di-parse, diubah ke huruf kecil, dan disalin di setiap request. `IntoCaseHeaderName`
  mendapat method `into_header_name_and_case()` (default-nya tetap mem-parse), dan tipe baru
  `CasedHeaderName` menyimpan nama yang sudah di-parse bersama bentuk tulisannya. conduit
  memakainya untuk `X-Forwarded-For`, `X-Real-IP`, `X-Forwarded-Proto`, `X-Forwarded-Host`, dan
  `x-request-id`. Bentuk tulisan semua header di wire tidak berubah.

### `9c64421`: kapitalisasi nama header tanpa map kedua

Pingora menyimpan bentuk tulisan setiap nama header (`Content-Type` vs `content-type`) supaya
header yang diteruskan tertulis persis seperti diterima. Caranya: HeaderMap kedua yang sejajar
dengan map nilai, dibangun saat parse, ikut di-clone, dan di-drop bersama header. Padahal nama
dalam satu pesan hampir selalu satu gaya: semua huruf kecil (hyper, banyak program era HTTP/2)
atau huruf besar di awal dan setelah `-` (nginx, curl, browser).

Sekarang header menyimpan gaya itu plus maksimal 8 nama yang tidak mengikutinya
(`pingora-http/src/name_case.rs`). Map penuh baru dibangun kalau pesan mencampur lebih banyak
gaya atau satu nama datang dalam dua bentuk, dan susunannya sama dengan map yang disimpan
sejak awal. Byte di wire sama persis; test acak 3000 putaran membandingkannya dengan cara lama.
Di profil, porsi `HeaderMap` turun dari 8,7% ke 6,3% sampel user, tapi throughput hampir tidak
bergerak: perbandingan nama untuk menentukan gaya memakan sebagian besar yang dihemat.

### `3357c9e`: satu flush per batch respons

`response_duplex_vec()` menulis header tanpa Content-Length, lalu setiap chunk, lalu chunk
terakhir, dan mem-flush setelah masing-masing. Respons kecil yang dikompresi jadi keluar
sebagai tiga `send()` dan tiga segmen TCP: header, body, `0\r\n\r\n` (terlihat dengan
LD_PRELOAD penghitung syscall). Sekarang batch di-flush sekali di akhir:

- Kalau batch mengakhiri stream, body, chunk terakhir, dan flush berjalan di bawah write
  timeout body.
- Kalau tidak, header yang minta di-flush (1xx, atau tanpa Content-Length seperti server-sent
  events) tetap di-flush sebelum kembali.
- Task tunggal (`response_duplex`) tetap di-flush seperti sebelumnya.

Test baru menghitung flush lewat `FlushTrackingMock`: respons chunked utuh 3 → 1 flush,
respons ber-Content-Length 2 → 1.

### `0a6aee4`: module HTTP tanpa indeks bersama

`HttpModuleCtx` mencari module (misalnya `ResponseCompression`) lewat `Arc<HashMap<TypeId,
usize>>` yang dipakai bersama semua context. Setiap request meng-clone `Arc` itu saat context
dibuat dan melepasnya di akhir, jadi semua request di semua thread menaikkan dan menurunkan satu
reference count yang sama; setiap `get`/`get_mut` juga meng-hash `TypeId` dengan SipHash. Satu
request hanya punya beberapa module, jadi sekarang module dicari berurutan dengan downcast, tanpa
state bersama. Pengecekan bahwa setiap tipe module hanya ditambahkan sekali tetap ada, dilakukan
saat context pertama dibuat. Sebelum patch, bagian module memakan 1,8% sampel worker.

### `bb3859d`: header respons upstream tanpa salinan

`read_response_task()` mengembalikan salinan header respons dan menyimpan aslinya. Proxy membaca
setiap respons upstream lewat fungsi ini, jadi setiap request membayar clone `HeaderMap`
(semua nama dan nilai, plus peta kapitalisasi) lalu membuang dua salinan.

Setelah header diserahkan, sesi hanya butuh sedikit darinya: framing body sudah disiapkan,
tinggal status dan keputusan keep-alive. Sekarang header respons final dipindahkan ke task.
Sesi mencatat status dan `header_keepalive()` (bagian header dari `respect_keepalive()`, dipisah
tanpa diubah) sebelum header pergi. `resp_header()` dan `get_header()` tetap menjawab: header
di-parse ulang dari byte mentah saat pertama diminta, dengan cara yang sama seperti
`read_response()`. Header 1xx tetap disalin karena sesi masih harus membaca lewat header itu.

Test membandingkan header yang diserahkan, header hasil parse ulang, dan hasil keep-alive dengan
`read_response()` untuk 9 kasus (keep-alive, timeout, close dari request atau respons,
HTTP/1.0, HTTP/1.0 chunked, TE bersama CL, nama header campuran dan berulang).

### `c4cce19`: channel task HTTP/1 sendiri

`proxy_1to1()` membuat dua channel `tokio::sync::mpsc` per request: body request ke paruh
upstream dan task respons kembali ke paruh downstream. Membuatnya (Arc berisi semaphore dan blok
pertama 32 slot), permit yang diambil dan dikembalikan di setiap putaran loop downstream,
kirim, terima, dan membuang receiver memakan 3,1% sampel worker.

Kedua paruh adalah future dalam satu task, jadi channel ini tidak pernah diperebutkan dan hanya
membawa beberapa task. `task_pipe` adalah ring 4 slot dalam satu alokasi, dengan bagian API mpsc
yang dipakai jalur H1 (`try_reserve`, `reserve`, `Permit`, `send`, `is_closed`, `recv`) dan
semantik yang sama:

- State dijaga spinlock kecil: ambil = satu compare-and-swap, lepas = store biasa. Versi pertama
  memakai `std::sync::Mutex`, yang melepas dengan swap atomik untuk mencari thread yang tidur, dan
  dua instruksi itu separuh biaya channel (porsi 2,5%).
- Tiap ujung menyimpan waker penunggunya plus flag, dan membangunkannya by reference, jadi
  menunggu lagi tidak meng-clone dan membuang waker.

Batasannya: satu task per ujung (di proxy keduanya di-poll oleh task yang sama). Jalur h2,
custom, dan subrequest tetap memakai channel tokio. Selain 6 unit test channel, test e2e
conduit `request_and_response_bodies` mengirim body 0 B sampai 3 MB dengan Content-Length dan
chunked ke upstream cepat dan lambat (backpressure), upload paralel, dan respons 20 MB.

## Test

Test Pingora butuh target directory tanpa spasi, karena jemalloc (dev-dependency Pingora) tidak
mau dibangun di path yang mengandung spasi:

```sh
cd vendor/pingora/pingora-core
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=$HOME/.cache/conduit-pingora-target cargo test --lib --features openssl
```

Target directory-nya di disk biasa, bukan `/tmp`: build test Pingora lebih dari 5 GB dan
memenuhi kuota tmpfs.

Yang dijalankan untuk fork ini: semua test unit `pingora-http`, `pingora-core` (dengan OpenSSL),
dan `pingora-proxy` (lib), ditambah unit test, e2e, ACME (Pebble), dan CrowdSec conduit. Test
integrasi `pingora-proxy` (`tests/`) tidak jalan di sini: test itu memakai `openresty` sebagai
origin.

## Memperbarui ke Pingora baru

```sh
cd vendor/pingora
git fetch --depth 1 origin tag 0.X.Y
git rebase --onto 0.X.Y 0.9.1 conduit
```

Lalu ubah versi di `Cargo.toml` conduit dan ukur ulang setiap patch dengan `scripts/ab.sh`.
Patch yang sudah diterima upstream dibuang saat rebase.

## Ke upstream

`git format-patch 0.9.1..conduit` menghasilkan satu file per patch, siap diajukan ke
cloudflare/pingora. Kandidat paling jelas: `32d98ca`, `ecef83b`, `9586c10`, `c448eff`,
`3357c9e`, dan `bb3859d`; juga `7ffe1c4` (perbaikan slowloris) dan `499f89d` (header `Server`). `c4cce19` memakai `unsafe` (spinlock) dan mengganti tipe channel di
jalur H1, jadi sebaiknya didiskusikan dulu.
`79bf391` mengubah perilaku pool (penutupan koneksi idle yang telat), jadi perlu didiskusikan
dulu dengan maintainer.
