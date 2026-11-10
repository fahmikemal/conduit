# Temuan terperinci

**Status perbaikan terbaru:** lihat [FIX_VALIDATION.md](FIX_VALIDATION.md). Uraian di bawah menyimpan kondisi dan nomor baris sebelum patch, bukan daftar bug yang masih terbuka.

**Diperbarui setelah verifikasi runtime:** lihat [RECHECK.md](RECHECK.md) untuk keputusan dan bukti terbaru. F01/F02/F03/F05/F06/F07/F09 terkonfirmasi; F04 ditarik sebagai bug; F08 sebagian valid/kontrak ambigu; F10 dokumentasi saja. Nomor baris merujuk checkout saat audit; [inventory.json](inventory.json) memuat hash. P1/P2/P3 adalah urutan penanganan bersyarat, bukan skor CVSS.

## F01 — P1: pemeriksaan plugin dan file lookup memakai decoding berbeda

**Lokasi:** `src/proxy.rs:588–624` (`run_plugins`), `src/proxy.rs:694–741` (`files_and_cache`), `src/files.rs:320–349` (`Files::find`), `src/pathnorm.rs:20–89`.

Urutan yang menentukan:

1. `run_plugins` menormalisasi raw path sebelum matcher rule/auth melihatnya.
2. `files_and_cache` membuat `Lookup.path` dengan normalisasi raw path yang sama.
3. `Files::find` mengembangkan `{path}` dari `Lookup.path`.
4. `pathnorm::normalize(&expanded)` melakukan percent-decoding lagi, bukan sekadar menyatukan segmen.

Contoh: raw `/%2561dmin/secret.txt` menjadi `/%61dmin/secret.txt` saat matcher. Prefix `/admin` tidak cocok. Saat file lookup, path berubah menjadi `/admin/secret.txt`. Default candidate `{path}` pun melewati jalur ini; tidak perlu konfigurasi try_files yang khusus.

**Prasyarat/dampak:** static file `/admin/secret.txt` ada dan aksesnya dibatasi oleh rule atau matcher auth untuk `/admin`. Request yang tidak cocok dengan proteksi dapat membuka resource tersebut. Tidak berarti bisa keluar root; ini bypass policy path di dalam root. `hide` dotfile tetap diperiksa setelah normalisasi kedua dan tidak menjadikan contoh `/admin` aman.

**Perbaikan:** pisahkan tipe raw/decoded path. Normalisasi persen hanya pada input raw. Expansion candidate harus menggunakan operasi penyusunan/normalisasi segmen yang tidak men-decode kembali byte `%`; tentukan escaping literal candidate pada compile time. Lebih baik `NormalizedPath` tidak dapat diserahkan ke decoder raw secara tidak sengaja.

**Regresi:** endpoint file dengan deny/auth prefix `/admin`; bandingkan `/admin/x`, `/%61dmin/x`, `/%2561dmin/x`, encoded slash/dot dan nama file yang memang mengandung `%61`. Semua keputusan harus merujuk resource yang sama dengan yang dibuka. Tambah properti lintas matcher/file, bukan hanya properti bentuk normalizer.

**Bukti terbaru:** binary asli menjawab 403 untuk path biasa dan single-encoded, tetapi 200 beserta file terlindungi untuk double-encoded. Lihat [hasil HTTP](recheck-runtime-results.json) dan [recheck F01](RECHECK.md). Model `reproduce.py` tetap merupakan bukti awal yang lebih lemah.

## F02 — P1 bersyarat: fallback containment rentan TOCTOU

**Lokasi:** `src/files.rs:591–641`; listing terpisah di `src/files.rs:957–980`.

Jalur utama menggunakan `openat2` dengan `RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS`, relatif terhadap FD root. Namun pada `ENOSYS` atau `EPERM`, flag global `NO_OPENAT2` mengalihkan proses ke:

```text
canonicalize(root / rel)
pastikan hasil starts_with(canonicalize(root))
OpenOptions.open(real)
```

Ada celah waktu setelah pemeriksaan dan sebelum pembukaan. Pihak yang dapat mengubah file/direktori di root bisa mengganti komponen path menjadi symlink keluar. Pembukaan biasa mengikuti symlink baru. `O_NONBLOCK` mencegah menunggu FIFO, tetapi tidak memberi containment. String path absolut hasil canonicalize tidak memegang inode yang diperiksa.

**Prasyarat:** openat2 tak tersedia/diblokir; actor lain dapat memutasi path di bawah root; target luar dapat dibaca user conduit. Ini bukan pembacaan luar root yang dapat dilakukan semua client hanya dengan URL pada root immutable.

**Perbaikan:** fail closed bila jaminan kernel wajib; atau implementasi traversal berbasis FD untuk setiap segmen, dengan kebijakan symlink eksplisit dan pembukaan final yang tidak mengikutinya. `O_NOFOLLOW` hanya pada komponen akhir tidak cukup jika komponen parent dapat diganti.

**Listing:** meskipun directory semula dibuka aman, `listing` membentuk path lalu `read_dir` kembali, tanpa memakai FD directory yang tadi diverifikasi. Directory dapat diganti pada sela ini. Dampak utamanya nama/ukuran/waktu entri; jangan menyamakannya dengan file-content read. Gunakan enumeration dari FD directory yang sudah dibuka dan metadata relatif terhadap FD, dengan kebijakan symlink.

**Regresi:** jalankan container yang memaksa fallback dan interleaving deterministik sesudah containment check; rename direktori parent maupun symlink file. Tidak boleh mendapat marker dari luar root. Uji listing dengan pergantian directory. Pengujian harus melibatkan interleaving, bukan sekadar symlink statis.

**Bukti terbaru:** fungsi Rust `open_beneath` diekstrak utuh dan dijalankan dengan fallback aktif. Race nyata membaca marker luar root pada percobaan 71; kontrol symlink statis ditolak. Tidak ada hook timing di badan fungsi. Lihat [hasil fallback](recheck-fallback.log). Deskripsi jaminan containment perlu menyebut batas fallback.

## F03 — P1 bersyarat: authority origin dan cache tidak identik

**Lokasi:** `src/proxy.rs:291–323`, `src/proxy.rs:1142–1191`, `src/proxy.rs:1343–1397`, `src/cache.rs:198–217`.

`request_host` membuang port serta menormalisasi casing/trailing dot. Itu wajar untuk routing site. Namun nilai ini juga menjadi komponen cache key. `upstream_request_filter` meneruskan Host original, termasuk port, dan menjadikannya X-Forwarded-Host ketika peer tidak terpercaya. Cache memasukkan X-Forwarded-Host mentah hanya pada cabang trusted peer.

Untuk peer biasa, `Host: app.test:8080` dan `Host: app.test:9090` mendapat `ctx.host = app.test`, scheme/URI sama dan identity tambahan kosong. Upstream menerima authority berbeda, cache primer sama. Bila origin menghasilkan redirect, absolute link atau konten berdasarkan authority, respons pertama dapat digunakan untuk request kedua.

**Dampak:** cache confusion/poisoning authority bersyarat. Kebocoran data pribadi tidak otomatis dapat disimpulkan. Origin harus menghasilkan respons yang dinilai cacheable dan kontennya benar-benar berbeda. `Vary: Host` atau `Vary: X-Forwarded-Host` dari origin dapat memisahkan varian; banyak aplikasi tidak menyatakan Vary untuk komponen host yang biasanya diharapkan sudah ada dalam key.

**Perbaikan:** gunakan representasi authority kanonis yang sama untuk forwarding/key, termasuk port yang bermakna. Pertimbangkan Host override dan effective X-Forwarded-Host secara bersama; jangan hanya menambahkan raw header yang justru tidak diteruskan. Tolak authority malformed sebelum routing.

**Regresi:** origin cacheable mengembalikan Host dan X-Forwarded-Host. Request dua port, default port eksplisit/implisit, trailing dot, HTTP/2 authority dan trusted/untrusted forwarding. Tidak boleh terjadi reuse lintas input yang menghasilkan representasi berbeda.

## F04 — DITARIK SEBAGAI BUG: cache hidup melintasi perubahan upstream

**Koreksi:** perilakunya terkonfirmasi lewat binary, tetapi sesuai kontrak TTL/purge pada `docs/configuration.md:1356` dan `:1390`. Tidak ada janji bahwa perubahan upstream menghapus entry fresh. Karena itu klasifikasi awal sebagai bug terlalu luas. Uraian berikut merupakan analisis perilaku dan usulan fitur opsional, bukan perbaikan wajib terhadap bug yang terbukti.

**Lokasi:** `src/reload.rs:121–149`, `src/cache.rs:401–403`, `src/proxy.rs:1343–1397`.

Saat reload, hanya site yang tidak lagi caching yang dibersihkan oleh `retain_sites`. Jika site ID tetap dan cache tetap aktif, entry dipertahankan. Key tidak memuat fingerprint konfigurasi site/pool. Route yang memilih pool memasukkan nama pool, tetapi bukan definisi/resolved backend; pool default site bahkan tidak dimasukkan sebagai komponen identity tambahan.

**Skenario:** site `app` meng-cache body upstream A dengan TTL panjang. Operator memindahkan upstream ke B atau mengganti upstream_host lalu reload. Request baru memegang runtime B, tetapi mendapat body A dari store global pada key lama. TTL/revalidasi atau purge manual akhirnya dapat mengatasinya; reload sendiri tidak.

**Implikasi operasional:** cutover upstream tidak langsung efektif untuk URL cached, konten tenant/lingkungan lama dapat tetap muncul jika operator memakai ulang site ID. Ini bukan bypass autentikasi otomatis: plugin auth tetap berjalan sebelum cache. Kebijakan cache yang didokumentasikan memang mempertahankan entry fresh; operator dapat memakai purge untuk cutover.

**Usulan opsional:** namespace cache dapat memakai generation/fingerprint stabil jika produk menghendaki invalidasi otomatis berdasarkan konfigurasi. Jangan generation setiap reload tanpa alasan, karena rotasi cert seharusnya tidak menghapus cache body. Kebijakan ini harus diputuskan sebagai perubahan kontrak, bukan diasumsikan sudah wajib.

**Jika fitur invalidasi otomatis dipilih:** tambahkan test warm A, reload B dan request in-flight. Untuk kontrak sekarang, recheck membuktikan A tetap served sampai purge, lalu B. Hasil itu tidak diperlakukan sebagai failure produk.

## F05 — P2: overflow duration tidak menjadi error konfigurasi

**Lokasi:** `src/units.rs:31–47`, khususnya perkalian di baris 41–43.

Ukuran byte memakai `checked_mul`, tetapi durasi menit/jam/hari memakai `n * konstanta`. Nilai valid u64 seperti `9223372036854775808m` melampaui range ketika dikalikan 60. Dengan overflow checking, terjadi panic; pada release biasa tanpa overflow checks hasilnya wrap menjadi nol. Contoh lain menghasilkan durasi kecil yang tidak dimaksudkan.

**Dampak:** `conduit check` tidak memberikan validasi numerik yang konsisten; input operator salah dapat mematikan proses/check, menghasilkan timeout salah atau memicu F06. Sumber input ini konfigurasi operator, bukan parameter request jarak jauh.

**Perbaikan:** `checked_mul` dan error terstruktur; validasi juga batas durasi ketika nanti dikali dua pada connect/probe atau ditambah ke Instant/SystemTime. Error parser seharusnya tidak membawa panic ke reloader.

**Regresi:** batas maksimum tiap unit, tepat di atas batas, u64::MAX, nol serta uji debug/release. Pertahankan dukungan `d` pada dokumentasi komentar yang saat ini hanya menyebut ms/s/m/h.

**Bukti terbaru:** native test memanggil fungsi library asli dan menangkap panic. Fungsi yang sama tanpa modifikasi, dikompilasi dengan `-O`, mengembalikan 0 detik. [Detail lingkungan dan output](recheck-environment.json).

## F06 — P2: watcher dengan periode nol lolos validasi

**Lokasi:** `src/config.rs:265–285`, `src/runtime.rs:371–548`, `src/main.rs:287–290`, `src/reload.rs:195`.

`Dur` menerima nol, dan build tidak memeriksa `tls.watch_interval > 0`. Startup menyerahkannya ke `tokio::time::interval(self.interval)`. [Dokumentasi resmi Tokio](https://docs.rs/tokio/latest/tokio/time/fn.interval.html) menyatakan pemanggilan ini panic jika periode nol.

**Dampak aktual setelah recheck:** check sukses, tetapi task `BG reload` panic saat startup. Proses HTTP tetap hidup dan menjawab 404 pada fixture tanpa site. Jadi ini kegagalan watcher, bukan bukti seluruh proses mati. Reload nilai ini tidak mengubah interval Reloader yang dibuat saat startup—opsi ini memang didokumentasikan perlu restart. [Log binary](recheck-runtime-results.json).

**Perbaikan:** tolak nol di semantic validation sebelum membuat services; tetapkan batas bawah yang wajar jika polling submilidetik tidak berguna.

**Regresi:** minimal YAML ber-listener dan `tls: {watch_interval: 0s}` harus gagal `conduit check` dengan pesan spesifik. Uji 1ms/1s sesuai kontrak yang dipilih.

## F07 — P2: keputusan rate limit tidak atomik dengan pencatatan

**Lokasi:** `src/limits.rs:157–170`.

`rate_with` membaca estimasi. Kode memeriksa `used + 1.0 > limit`, lalu memanggil `observe` terpisah. Tidak ada guard yang mengikat kedua operasi. Dua worker bisa membaca kuota terakhir yang sama dan keduanya lolos sebelum observasi tercatat.

**Dampak:** limit merupakan pendekatan saat concurrency tinggi, bukan batas admission ketat. Count-min sketch yang overestimate tidak menutup race di luar sketch. Request sequential dalam satu task tidak membuktikan bebas race antarworker; batas overshoot aktual belum diukur.

**Perbaikan:** tentukan dahulu kontrak. Jika wajib ketat, operasi reservation+decision harus terkoordinasi per key/shard/window; desain rollback pada sketch perlu dianalisis agar tidak menciptakan undercount. Jangan sekadar menambah satu mutex global yang merusak throughput semua site. Jika sengaja approximate, dokumentasikan toleransi dan sediakan concurrency limiter sebagai kontrol terpisah.

**Bukti terbaru:** 16 thread memakai `RateLimit::request` asli, tanpa timing hook/modifikasi implementasi. Trial pertama meloloskan 4 request untuk limit 1/3600s. Source `rate_with`/`observe` pada fork terkunci juga diperiksa: tidak ada reservation lintascall. Ini race admission, bukan bukti estimator menghitung salah. [Log native](recheck-native.log).

## F08 — SEBAGIAN VALID: override wildcard ambigu; warning coverage terbukti salah

**Koreksi kontrak:** binary memang memilih cert lama untuk host wildcard dan chosen untuk exact control. Namun selection reference `docs/configuration.md:1399` secara eksplisit menyebut exact host, sedangkan deskripsi field `:475` menyebut these hosts. Karena kontraknya ambigu, tidak tepat menyebut bagian override wildcard sebagai bug keamanan yang pasti. Bug warning `covers` di bawah terkonfirmasi dengan native test dan bersifat lebih kecil. Lihat [recheck F08](RECHECK.md).

**Lokasi:** `src/runtime.rs:451–458`, `src/tls.rs:139–147`, `src/tls.rs:173–189`.

`add_override("*.example.test", "chosen")` menyimpan key literal `*.example.test` dalam map overrides. Lookup untuk SNI `app.example.test` hanya mencoba `overrides.get(host)` dan exact SAN. Fallback wildcard mencari map SAN berdasarkan parent domain, bukan map override wildcard.

**Dampak:** site wildcard dengan `cert: chosen` tidak memaksa sertifikat pilihan untuk subdomain konkret. Jika ada sertifikat lain yang sudah mengisi SAN wildcard lebih dulu, cert tersebut yang digunakan. Bila keduanya masih valid untuk domain sama, koneksi bisa tetap berhasil sehingga bug tersembunyi sampai pergantian key/chain/expiry.

**Klarifikasi/perbaikan:** nyatakan apakah override wildcard didukung. Jika didukung, buat lookup dan precedence eksplisit; jika exact-only, tolak opsi wildcard yang tak bermakna atau jelaskan batasnya. Selaraskan cakupan TLS satu label dengan router suffix. Bagian ini merupakan penyelesaian kontrak, bukan bukti bypass TLS.

**Bug yang terkonfirmasi:** `covers("*.example.test")` membuang `*.` lalu memotong label sekali lagi sehingga pencarian parent dapat menjadi `test`. Native test menghasilkan warning `no certificate covers *.example.test` meskipun cert wildcard valid dipilih untuk host konkretnya. Perbaiki pengecekan pola. Ini warning false-positive, bukan kegagalan TLS.

**Regresi:** dua cert dengan SAN wildcard sama, konfigurasi global cert lama dulu lalu chosen, site wildcard override chosen. Pastikan SNI konkret mendapat chosen; sertakan exact SAN yang tumpang tindih dan nested hostname. Uji `covers` terhadap wildcard config serta host biasa.

## F09 — P2: variance Accept-Encoding kehilangan coding tak dikenal

**Lokasi:** `src/cache.rs:139–182`, `src/files.rs:84–140`.

Ketika origin menyatakan `Vary: Accept-Encoding`, cabang `listed` memakai `Accepted::bits()` untuk seluruh daftar coding. Bitset hanya mengenal br, zstd dan gzip. Maka `Accept-Encoding: deflate` dan `Accept-Encoding: identity` keduanya direpresentasikan nol.

Jika origin mengirim `Content-Encoding: deflate` dan Vary tersebut, kedua request berpotensi mendapat variance yang sama. Cabang fallback raw-header untuk content coding tak dikenal hanya berlaku pada kasus `!listed`; tidak menolong kasus Vary yang eksplisit.

**Dampak:** client yang tidak meminta deflate dapat menerima body deflate dari cache. Ini bukan pembuktian kebocoran data; utamanya correctness content negotiation. Penyederhanaan juga mengabaikan bobot q nonnol dan extension coding lain; request yang menerima banyak coding belum tentu memilih representasi sama, walaupun keduanya bisa mendekode salah satunya.

**Perbaikan:** canonicalization harus mempertahankan seluruh token coding dan nilai q yang bermakna, atau gunakan semua nilai header asli sebagai variance konservatif. Pertahankan perbedaan absent/empty bila origin membedakannya. Optimisasi varian memerlukan bukti kesetaraan, bukan hanya kesamaan tiga bit.

**Regresi:** origin yang memilih deflate/identity dengan `Vary: Accept-Encoding`; warm satu lalu minta lainnya. Tambahkan coding extension, beberapa field Accept-Encoding, wildcard, q=0, q dengan prioritas berbeda, absent dan empty. Verifikasi decoding payload, bukan hanya header Cache-Status.

**Bukti terbaru:** HTTP asli mendapat cache hit deflate untuk request identity; control URI baru menghasilkan identity. Native variance callback juga menghasilkan collision. [Hasil binary](recheck-runtime-results.json) dan [log native](recheck-native.log).

## F10 — P3: kontrak Server header berbeda antara README dan implementasi

**Lokasi:** `README.md:31–32`, `src/config.rs:176–187`, `src/proxy.rs:758–768`.

README menyatakan `server_header: off` tidak mengirim Server header sama sekali. Komentar config dan kode menyatakan conduit berhenti menambahkan Server, tetapi Server milik origin tetap lewat. `decorate` tidak memanggil remove_header ketika nilai None. Ini ketidaksesuaian dokumentasi; belum tentu bug implementasi karena kontrak config yang lebih rinci memang memilih pass-through.

**Koreksi keputusan:** existing e2e `server_header` secara eksplisit mengharapkan pass-through, dan binary recheck melakukan hal itu. Ini **bug README saja**, bukan bug implementasi. Rekomendasi utama adalah membetulkan README. Mengubah off menjadi strip-all merupakan perubahan kontrak yang terpisah.

**Regresi:** origin mengirim `Server: origin-marker`; bandingkan default, off, custom dan plugin remove. Sertakan response lokal dan error Pingora, dengan catatan error fork baru bisa diuji setelah vendor tersedia.

## Observasi lanjutan: perlu desain/tes, bukan semuanya bug

1. **`present` pada header memang berarti tidak kosong.** `rules.rs` memakai `!value.is_empty()`, dan docs/configuration.md menjelaskan missing-or-empty. Ini diperiksa dan sengaja **tidak** dilaporkan sebagai bug presence HTTP.
2. **force_https dan scheme matcher memakai koneksi lokal.** Trusted X-Forwarded-Proto dipakai saat forwarding/cache key/auth scheme. Dokumentasi force_https menyebut plain connection, jadi jangan langsung menyatakan redirect loop sebagai bug. Jelaskan topologi TLS termination kepada operator.
3. **X-Forwarded-For memelihara nilai kiriman client.** Kode menambah resolved client IP, juga untuk peer untrusted. Ini perilaku mirip proxy_add_x_forwarded_for; origin harus menggunakan trusted-proxy chain parsing. X-Real-IP ditimpa secara aman. Tidak otomatis berarti IP resolver conduit dapat di-spoof.
4. **Auth yang sama tidak selalu memberi view request hasil auth ke plugin berikutnya.** `upstream_edits` baru diaplikasikan di upstream_request_filter; matcher/header limiter berikutnya masih membaca request asli. Ini kontrak penting saat ingin rate-limit berdasarkan Remote-User: header client bukan hasil forward_auth.
5. **AppSec tidak memeriksa semua body.** Chunked, Expect: 100-continue dan body melebihi body_limit dikirim tanpa body ke inspector; hal ini didokumentasikan. Default failure_action adalah allow. Ini batas kemampuan, bukan klaim WAF lengkap.
6. **Hot reload bukan satu transaksi global.** CertStore, runtime, server header dan cache capacity diganti terpisah. Request memegang runtime konsisten, tetapi status endpoint/handshake lintas fase bisa melihat generasi berbeda sementara. mTLS trust ID mengurangi risiko salah pakai; belum terbukti bypass. Pertimbangkan bundle snapshot jika memerlukan publikasi atomik lintas state.
7. **Reloader melakukan operasi blocking pada service async.** Build, DNS to_socket_addrs, file read dan sebagian crypto dapat memperlambat service tersebut. Pingora memakai service runtime terpisah menurut konfigurasi; jangan menyatakan worker request pasti ikut terblokir tanpa audit fork. Shutdown watcher lambat masih perlu diuji.
8. **Disk error log bukan queue overflow.** AccessLog membuang buffer yang gagal ditulis tanpa retry durable, sedangkan counter dropped khusus queue penuh. Tambahkan counter write_error/open_error dan telemetry kehilangan baris; bukan jaminan audit log durabel. Queue dibatasi jumlah baris, bukan total byte.
9. **Health fallback fail-open disengaja.** Saat tidak ada backend sehat, select mencoba yang tidak excluded. Cocok untuk availability ketika probe salah, tetapi passive ejection bukan karantina keras. Respons 5xx origin juga dianggap sukses koneksi dan tidak otomatis passive failure.
10. **Least_conn menghitung request in-flight, bukan socket.** Lease bertahan selama request, termasuk koneksi upgraded; ini perlu dipahami saat membaca metrik dan bobot backend.
11. **TLS wildcard router dan sertifikat berbeda cakupan.** Router suffix mengizinkan subdomain lebih dalam, wildcard X.509 hanya satu label. Config wildcard membutuhkan test coverage sertifikat sesuai host nyata.
12. **ACME account memoization tidak memuat storage path.** Manager meng-key account dengan directory URL dan ca_file saja. Jika storage berubah saat reload, account lama masih di-memory; perlu uji perilaku persistence/account.json di storage baru. Ini observasi audit, belum tes runtime.
13. **Cache purge bukan generation barrier.** Miss in-flight dapat mengisi ulang setelah purge; definisikan apakah endpoint menjanjikan sekadar drop entries saat itu atau invalidasi kuat.
14. **Failed build dapat menyentuh registry metrics.** Beberapa plugin mendaftarkan label sebelum seluruh build selesai. Reload gagal berulang dengan ID baru dapat menyisakan label; perlu pengukuran cardinality dan cleanup transaksi. Counter backend historis juga tidak dibersihkan seperti gauge scrape-time.
15. **ACME deadline total belum seragam.** Forward-auth dan CrowdSec membungkus call dengan timeout total; adapter ACME memakai batas per operasi dari httpc. Respons yang terus mencicil data dapat memperpanjang call. CA adalah dependency terpercaya; uji failure mode, bukan simpulkan SSRF.
16. **Harness remote soak melakukan mutasi host.** deploy.sh menghentikan unit, dapat memasang/menonaktifkan nginx dan memodifikasi nftables pada host disposable. Audit ini tidak menjalankannya. Tambah trap cleanup pada soak.sh jika harness harus tahan abort.
17. **Benchmark lintas-proxy belum merupakan gate konsisten.** compare.py menghitung dan mencetak errors serta log line totals, tetapi tidak seluruhnya dijadikan exit failure; bench.sh mempunyai regression gate lebih jelas. Tags mutable juga mengurangi reproduktibilitas. Simpan image digest, compiler, kernel/CPU, argv dan raw output.
18. **Docker scratch adalah export artefact.** Image akhir hanya berisi binary dengan glibc dinamis dan tanpa entrypoint. Komentar Dockerfile menyatakan buildx output local; jangan laporkan sebagai runtime-image bug. Jika hendak menyediakan image runnable, itu target deployment baru yang perlu loader/CA store/entrypoint.
19. **Tidak ada workflow CI tracked dalam snapshot ini.** mise tasks tersedia, tetapi itu tidak membuktikan pemeriksaan otomatis dijalankan sebelum merge. Tambahkan CI bila memang belum dilakukan lewat sistem di luar repository.
20. **Rule engine mempunyai batas 64 signature.** ENGINE assert jumlah Def <=64 dan memakai u64 mask. Menambah signature tanpa mengubah representasi dapat menyebabkan panic saat engine pertama dipakai. Jadikan invariant ini test eksplisit/build-time check.
