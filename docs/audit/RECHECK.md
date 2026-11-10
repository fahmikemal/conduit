# Verifikasi ulang: bug, risiko bersyarat, atau false positive?

**Sesudah perbaikan:** hasil closure dan suite terbaru berada di [FIX_VALIDATION.md](FIX_VALIDATION.md). Dokumen ini mempertahankan hasil recheck pada binary sebelum patch.

**Hasil:** tujuh temuan implementasi terkonfirmasi dengan eksekusi Rust/binary asli (F01/F02/F03/F05/F06/F07/F09). F04 ditarik sebagai bug karena perilaku itu sesuai kontrak cache. F08 harus dipisahkan antara perilaku override yang kontraknya ambigu dan bug warning yang terbukti. F10 hanya ketidaksesuaian README, bukan bug implementasi.

Audit awal terlalu luas ketika mengelompokkan semuanya sebagai temuan bug. Recheck ini menjadi rujukan terbaru untuk status; uraian historis di laporan awal bukan bukti runtime tambahan.

## Matriks keputusan akhir

| ID | Keputusan | Bukti utama | Batas kesimpulan |
|---|---|---|---|
| F01 | **Bug nyata: bypass policy path** | HTTP asli: `/admin/secret.txt` → 403; `/%61dmin/secret.txt` → 403; `/%2561dmin/secret.txt` → 200 dan isi file terlindungi | Butuh static files dan proteksi berdasarkan path; bukan traversal keluar root |
| F02 | **Bug keamanan bersyarat: TOCTOU fallback** | Fungsi Rust `open_beneath` asli membaca marker luar root pada percobaan 71 saat replacement bersamaan; kontrol symlink statis ditolak | Flag fallback dipaksa aktif sebagai simulasi ENOSYS/EPERM; actor harus bisa mengganti file di root |
| F03 | **Bug nyata: authority cache collision** | Request Host `cache.test:9090` mendapat cache hit berisi Host/XFH `cache.test:8080`; control tanpa entry lama menghasilkan 9090 | Origin memakai authority dalam body dan mengizinkan caching |
| F04 | **Ditarik sebagai bug / false positive klasifikasi** | Body A tetap di-cache setelah reload B, lalu purge menghasilkan B; docs secara eksplisit memakai TTL dan hanya membuang cache site yang tidak lagi caching | Invalidasi pada cutover dapat menjadi fitur tambahan; tidak ada janji semua reload menghapus fresh cache |
| F05 | **Bug nyata: unchecked overflow** | Fungsi asli panic pada dev; fungsi yang sama dikompilasi teroptimisasi mengembalikan 0 detik | Input konfigurasi operator ekstrem, bukan request remote biasa |
| F06 | **Bug nyata: validasi watcher** | `conduit check` sukses; startup log menunjukkan panic pada `src/reload.rs:195` | Task reload mati, proses HTTP tetap hidup dan menjawab 404; bukan process-wide crash pada percobaan ini |
| F07 | **Race nyata: admission melampaui limit** | `RateLimit::request` asli meloloskan 4 dari 16 request bersamaan untuk limit 1/3600s pada trial pertama | Ini gap admission, bukan bukti count-min sketch menghitung salah; jika toleransi race dimaksudkan, kontraknya perlu dijelaskan |
| F08 | **Sebagian valid; klaim awal perlu dipersempit** | TLS nyata memilih cert lama untuk wildcard dan chosen untuk exact; native runtime juga menghasilkan warning coverage palsu | Override wildcard bertentangan antara field description “these hosts” dan selection rule “exact host”; bug warning jelas, niat override wildcard belum tegas |
| F09 | **Bug nyata: salah varian encoding** | Client `Accept-Encoding: identity` mendapat `Content-Encoding: deflate`, body zlib yang dapat didekode ke marker origin-deflate, dan Cache-Status hit | Respons origin memakai Vary: Accept-Encoding; compression conduit dimatikan untuk mengisolasi cache |
| F10 | **Bug dokumentasi saja** | Binary off meneruskan Server origin; existing e2e memang secara eksplisit mengharapkan pass-through | README terlalu absolut; implementation dan konfigurasi rinci selaras |

Perilaku yang dapat direproduksi belum tentu bug: F04 dan F10 adalah contoh mengapa hasil test harus dibandingkan dengan kontrak yang dijanjikan.

## Lingkungan dan integritas sumber

- Compiler: **rustc 1.99.1**, host x86_64 Linux.
- Manifest dan `Cargo.lock` asli, build `--locked`, default features tetap aktif.
- Fork Pingora di-checkout pada **5c765c66590bdb7829ad32e00abc6f9a4e0410af**, sama dengan gitlink repository. Tidak memakai HEAD terbaru fork.
- Salinan build: `/tmp/conduit-audit-work`. Toolchain dan dependensi build yang kurang disiapkan di area sementara; tidak memasang paket sistem.
- Build **dev**, debug info dimatikan untuk mengurangi resource. OpenSSL tetap vendored sesuai manifest; tidak dialihkan ke libssl sistem.
- Hash semua 29 file `src/*.rs`, Cargo.toml dan Cargo.lock pada build copy sama dengan proyek. Tidak ada perubahan implementasi untuk memaksa hasil test.
- Native integration tests ditambahkan hanya pada `tests/audit_recheck.rs` di salinan sementara. Pengujian fallback mengompilasi fungsi produksi yang diekstrak utuh, bukan terjemahan Python.
- Server pengujian hanya mendengarkan loopback; root file, origin dan sertifikat merupakan fixture disposable. Tidak mengakses production atau sertifikat pengguna.

Detail compiler, hash dan hasil overflow: [recheck-environment.json](recheck-environment.json). Hash binary dan response/log aktual: [recheck-runtime-results.json](recheck-runtime-results.json).

## Bukti per temuan

### F01: ada kontrol yang membedakan bug dari konfigurasi terbuka

Konfigurasi test menggunakan plugin rule deny prefix `/admin` dan root berisi `admin/secret.txt`. Tidak ada try_files khusus, passthrough upstream atau exception auth. Request biasa dan single-encoded ditolak. Hanya double-encoded yang membuka file dan mengembalikan marker `protected-file-marker`.

Ini membuktikan keputusan plugin dan resource yang dibuka berbeda pada pipeline nyata. Model Python audit awal hanya menunjukkan dugaan algoritme; hasil HTTP ini mengonfirmasi integrasinya.

Lokasi: `src/proxy.rs:588`, `src/proxy.rs:694`, `src/files.rs:340`.

### F02: fallback rentan race, tetapi symlink biasa bukan bypass

Dua kontrol membuka symlink ke luar root tanpa pergantian bersamaan, dan keduanya ditolak. Test kemudian mengaktifkan `NO_OPENAT2` untuk mensimulasikan fallback yang sudah dipilih setelah ENOSYS/EPERM. Satu thread mengganti path `item` antara regular file dan symlink; thread lain memanggil **fungsi produksi tanpa modifikasi**.

Hasil pembacaan file luar berhasil pada percobaan 71. Tidak ada sleep atau hook di tengah canonicalize/open untuk memaksanya. Interleaving terjadi lewat scheduler/filesystem nyata. Semua file, termasuk “luar root”, tetap berada di direktori sementara milik test.

Ini tidak membuktikan URL biasa dapat keluar root pada sistem yang memakai jalur `openat2`. Prasyarat actor dapat menulis/mengganti path dan fallback aktif harus tetap disebutkan.

Kode test: [recheck_fallback.rs](recheck_fallback.rs). Hasil: [recheck-fallback.log](recheck-fallback.log). Lokasi produksi: `src/files.rs:591–641`.

### F03: miss pertama, hit salah authority, dan control baru

Origin echo mengembalikan Host serta X-Forwarded-Host yang benar-benar diterimanya dan `Cache-Control: public, max-age=600`. Peer localhost tidak dimasukkan trusted real-IP CIDR.

1. Host `cache.test:8080`, URI `/authority`: respons menyebut 8080 dan tersimpan.
2. Host `cache.test:9090`, URI sama: **cache hit**, tetapi body masih menyebut 8080.
3. Host `cache.test:9090`, URI control baru: origin benar-benar melihat 9090.

Jadi hasil bukan karena origin mengabaikan port atau karena client mengirim Host salah. Key menggabungkan input yang menghasilkan representasi berbeda. Response tidak mempunyai Vary Host karena host/authority seharusnya sudah menjadi komponen identitas resource cache.

Lokasi: `src/proxy.rs:291`, `src/proxy.rs:1142`, `src/proxy.rs:1343`.

### F04: perilaku nyata, tetapi klasifikasi bug ditarik

Test menghangatkan body `origin-A`, mengubah upstream ke B, mengirim SIGHUP dan menunggu `/status` benar-benar menampilkan backend B. Request yang sama masih mendapat A dari fresh cache; purge menghasilkan B.

Kontrak di `docs/configuration.md:1356` menyatakan fresh entry disajikan tanpa bertanya ke upstream. Bagian purge `docs/configuration.md:1390` menyatakan reload membuang cache site yang tidak lagi memiliki cache. Tidak ada janji perubahan upstream membatalkan setiap entry fresh.

Karena itu **tidak cukup dasar menyebut ini bug**. Namespace berdasarkan semantik konfigurasi boleh direkomendasikan sebagai fitur/kebijakan cutover, tetapi bukan perbaikan wajib atas pelanggaran kontrak yang sudah terbukti. Skenario reuse site ID untuk tenant berbeda tetap memerlukan purge oleh operator sesuai kebijakan cache.

### F05: overflow diuji dengan Rust, termasuk mode teroptimisasi

`parse_duration("60m")` menghasilkan 3600 detik sebagai kontrol. Input `9223372036854775808m` dipanggil melalui library asli pada native test: panic multiplication overflow tertangkap oleh catch_unwind.

Untuk memeriksa perbedaan codegen tanpa membangun seluruh app release, badan fungsi yang sama diekstrak tanpa perubahan dan dikompilasi dua kali: dengan `-C overflow-checks=yes` serta dengan `-O`. Yang pertama panic, yang kedua mencetak `returned_seconds=0`. Ini lebih kuat daripada model modulo Python, meskipun bukan build release seluruh aplikasi.

Lokasi: `src/units.rs:41`. Nilai ukuran memakai checked_mul, tetapi durasi unit m/h/d belum.

### F06: dampak aktual adalah task reload berhenti

Konfigurasi nol lulus `conduit check` dengan return code 0. Ketika binary dijalankan, log menunjukkan:

```text
thread 'BG reload' ... panicked at src/reload.rs:195:24:
`period` must be non-zero.
```

Proses masih hidup dan listener HTTP menjawab 404 untuk host tak dikenal. Maka deskripsi “proses pasti mati” akan salah; masalah yang terbukti adalah konfigurasi dinyatakan valid tetapi service reload tidak berjalan. Ini lebih berbahaya secara observability daripada sekadar startup ditolak dengan jelas.

Context7 kembali mengonfirmasi kontrak [`tokio::time::interval`](https://docs.rs/tokio/latest/tokio/time/fn.interval.html): periode nol panic. Log binary menentukan cakupan dampak aktual.

### F07: bukan asumsi tentang internal sketch

Native test memakai public `RateLimit::request` dari library asli. Enam belas thread menunggu barrier **sebelum masuk fungsi** lalu menggunakan IP yang sama. Tidak ada perubahan function body, delay atau hook setelah pembacaan allowance. Dengan limit 1 per satu jam, trial pertama mengizinkan 4 request.

Source fork pada commit terkunci memperlihatkan `Rate::rate_with` hanya membaca counter lalu mengembalikan nilai, sedangkan `observe` meningkatkan counter pada call terpisah. Tidak ada reservation lock lintascall yang tersembunyi. Counter sketch tetap dapat benar sesudahnya; yang salah terhadap batas admission ketat adalah beberapa request sudah terlanjur lolos.

Temuan ini valid sebagai concurrency correctness issue, tetapi bukan klaim penyerang mendapatkan allowance tak terbatas. Besar overshoot bergantung interleaving/worker. Count-min sketch yang approximate juga tidak otomatis berarti semua race admission telah dijanjikan sebagai toleransi.

Lokasi caller: `src/limits.rs:159–170`. Source dependency: `pingora-limits/src/rate.rs:137` (`observe`) dan `:191` (`rate_with`) pada fork terkunci; nomor baris dapat ditelusuri dari checkout dependency.

### F08: bedakan dua klaim

**A. Override wildcard:** dua sertifikat mempunyai SAN `*.example.test`; cert lama didaftarkan dulu. Site wildcard menggunakan `cert: chosen`. Handshake untuk `app.example.test` mendapat cert lama; site exact control mendapat chosen.

Perilaku ini nyata. Namun `docs/configuration.md:1399` menjelaskan override untuk **exact host**, sedangkan tabel field `:475` dan komentar `src/config.rs:438` menyebut sertifikat untuk “these hosts” tanpa membatasi wildcard. Jadi tidak jujur menyatakannya sebagai pelanggaran kontrak yang tidak ambigu. Klasifikasi terbaru: **kontrak ambigu/dukungan wildcard tidak jelas**, tanpa klaim bypass TLS. Pilihan aman API adalah mendukung wildcard override atau menolak konfigurasi yang tidak didukung, bukan membiarkan arti opsi kabur.

**B. Coverage warning:** native test membuat satu wildcard certificate valid, tanpa site cert override. `covers("app.example.test")` true tetapi `covers("*.example.test")` false. `runtime::build` kemudian memberi warning `no certificate covers *.example.test`, walau certificate itu dipilih untuk host konkret di pola tersebut. Ini **bug warning nyata**, berdampak kecil; bukan kegagalan handshake atau kebocoran key.

Karena itu label F08 tidak boleh lagi menyamakan warning false-positive dengan kegagalan keamanan TLS.

### F09: body salah encoding, bukan hanya hash yang tampak sama

Origin memilih deflate atau identity dan menyatakan `Vary: Accept-Encoding`. Compression conduit dimatikan agar tidak ada transformasi tambahan.

- Request pertama `Accept-Encoding: deflate` menyimpan body zlib.
- Request kedua `Accept-Encoding: identity` mendapat `Cache-Status: conduit; hit`, `Content-Encoding: deflate`, dan body yang hanya benar setelah zlib.decompress.
- URI control dengan identity mendapat body plain `identity-origin`.

Native test juga memanggil `cache::variance` asli dan menemukan hash sama untuk deflate/identity, sedangkan control gzip berbeda. Source fork memanggil callback itu untuk fill maupun hit di `pingora-proxy/src/proxy_cache.rs:108` dan `:685`. Tidak ada penjaga framework lain yang menyelamatkan kasus ini pada binary yang diuji.

Lokasi aplikasi: `src/cache.rs:139–182`, `src/files.rs:87–120`.

### F10: implementasi sesuai test, README yang perlu diperbaiki

Binary dengan server_header off tetap mengirim `Server: origin-marker`. Existing test `server_header` di `tests/e2e.rs:5132` secara eksplisit mengharapkan Server origin diteruskan. Komentar konfigurasi rinci juga menjelaskan hal yang sama.

Temuan tetap valid sebagai **dokumentasi yang menyesatkan**, tetapi bukan implementation bug. Rekomendasi utama adalah menyelaraskan README; mengganti implementasi menjadi strip-all justru mengubah kontrak/test yang telah ada.

## Pengujian yang dijalankan

- Build binary asli pada fork dan lockfile terkunci: berhasil.
- Empat native recheck tests: **4 passed**, mencakup overflow, race limiter, warning wildcard dan variance.
- Satu test fungsi fallback yang diekstrak utuh: **1 passed**, membuktikan race positif dan kontrol symlink negatif.
- Harness HTTP/TLS: tujuh skenario selesai dengan assertion/control, termasuk dua perilaku yang tidak dikategorikan implementation bug.
- Existing library tests: **109 passed, 2 ignored, 0 failed**. Tidak mengklaim ignored tests dijalankan.
- Existing e2e `file_server`: **1 passed**.
- Existing e2e `response_cache`: **1 passed**.

Hasil existing e2e `response_cache` dicatat pada [recheck-e2e-cache.log](recheck-e2e-cache.log). Status final juga dirangkum di [recheck-validation.json](recheck-validation.json). Tidak ada full-suite e2e, fuzz/soak baru, MSRV test, clippy atau benchmark baru yang diklaim.

Hasil positif suite lama tidak membatalkan temuan: skenario double encoding, authority port dan coding deflate ini memang belum diliput oleh test yang lulus tersebut.

## Artefak reproduksi

- [Harness HTTP/TLS](recheck_runtime.py): dijalankan terhadap binary apa pun yang hendak diverifikasi, tanpa mengubah aplikasinya.
- [Native tests](recheck_native.rs): salin ke `tests/audit_recheck.rs` dalam checkout disposable lalu jalankan `cargo test --test audit_recheck -- --nocapture --test-threads=1`.
- [Test fallback](recheck_fallback.rs): test private helper, untuk salinan disposable atau wrapper yang mengekstrak fungsi utuh. Memaksa flag fallback hanya dalam fixture test.
- [Hasil native](recheck-native.log), [hasil fallback](recheck-fallback.log), [hasil existing unit](recheck-unit.log), [hasil existing file e2e](recheck-e2e-files.log).

Prioritas perbaikan yang masih kuat: **F01 dan F03**, disusul **F09**, **F02 pada root mutable/fallback**, serta validasi durasi/watcher dan admission race. F04 bukan alasan mengubah arsitektur cache tanpa keputusan produk; F08/F10 perlu penanganan yang sesuai cakupan kecil/kontraknya.
