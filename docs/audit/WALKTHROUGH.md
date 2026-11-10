# Bedah folder dan modul

**Status pada implementasi `03cae2b`, 10 November 2026:** perbaikan temuan dan HTTP/2
upstream sudah diterapkan. Suite final: 111 unit dan 34 E2E lulus, ditambah satu regresi
native pool Pingora. [Indeks audit](README.md) dan [laporan H2](HTTP2_UPSTREAM_VALIDATION.md)
menjadi rujukan status sekarang.

## Perubahan sejak snapshot audit

| File/modul | Perilaku saat ini |
|---|---|
| `src/config.rs` | `UpstreamHttpVersion`, `http_version` dan `http2_max_streams`; default H1.1, required H2 atau TLS auto. |
| `src/upstream.rs` | Resolusi h2c, `Target::peer`, signature protokol/stream limit, health probe H2 dan canonical Host IPv6. Probe memakai satu stream dan idle timeout nol. |
| `src/proxy.rs` | Enforcement required H2 sebelum request aplikasi, pool terpisah per protokol, HTTP/1 upgrade pada auto, authority berport dalam cache key. Group H1 per worker mempertahankan identitas lama. |
| `src/pathnorm.rs`, `src/files.rs` | Candidate path tidak di-percent-decode dua kali; fallback berjalan melalui pinned FD dengan `O_NOFOLLOW`; listing memakai FD dan mengabaikan symlink. |
| `src/cache.rs` | Explicit Vary memakai nilai/header field asli, termasuk coding tak dikenal dan absent/empty. Fresh cache tetap bertahan setelah reload upstream sampai TTL/purge. |
| `src/limits.rs`, `src/units.rs`, `src/runtime.rs` | Admission same-key diserialisasi pada 64 shard; perkalian durasi diperiksa; watch interval nol/tidak representable ditolak sebelum publikasi runtime. |
| `src/tls.rs` | Exact override lalu wildcard override satu label, diikuti exact SAN/wildcard SAN/default; coverage wildcard diperbaiki. |
| `src/admin.rs` | Backend status menyertakan configured `http_version`; `auto` merupakan policy, bukan hasil negosiasi terakhir. |
| `tests/e2e.rs` | Delapan regresi `security_` dan modul support H2. |
| `tests/support/h2_upstream.rs` | Origin H1/H2/TLS nyata, lima tes fungsional dan satu soak 120 detik yang ignored secara default. gRPC dua arah, frame 2 MiB, trailers, cancellation dan reload ikut diuji. |
| `vendor/pingora-h2-reuse.patch` | Backport PR1025: lock/release sebelum reinsertion agar entry InUsePool tidak terduplikasi; termasuk tes race deterministik. |
| `scripts/prepare-vendor.sh`, Dockerfile, mise, PGO | Patch lokal dipersiapkan idempotently; native Cargo langsung memerlukan persiapan terlebih dulu. Gitlink fork tetap terkunci. |

Client-facing h2c dan RFC8441 belum diimplementasikan. Native gRPC memakai HTTP/2 melalui
listener TLS conduit. Required H2 tanpa ALPN menghasilkan 502 sebelum request dikirim;
forwarded HTTP/1 upgrade pada required H2 ditolak 400. Lihat [referensi pool](../configuration.md#4-upstream-pools).

## Panduan membaca bedah historis

Uraian berikut menjelaskan source baseline `a6b2bb9` sebelum perbaikan. Nomor baris, jumlah
tes, dugaan risiko dan pernyataan “belum dijalankan” dipertahankan untuk menelusuri asal
analisis; gunakan tabel di atas dan source checkout saat ini untuk perilaku sesudah patch.
F04 ditarik sebagai bug, F08 telah dipertegas, dan F10 hanya koreksi dokumentasi. Snapshot
lengkap baseline ada di [SOURCE.html](SOURCE.html) dan [inventory.json](inventory.json).

Laporan ini dibaca bersama [temuan](FINDINGS.md). Referensi `file:baris` merujuk sumber saat snapshot; penjelajah [SOURCE.html](SOURCE.html) menyediakan baris lengkap. Penjelasan dibagi berdasarkan blok logika agar keputusan desain, aliran data dan dampak saling terhubung. Inventaris simbol otomatis terdapat di [INVENTORY.md](INVENTORY.md); daftar simbol bukan analisis semantik tambahan.

## Folder root: kontrak build, distribusi dan dokumentasi

### Cargo.toml

Manifest mendefinisikan binary/library privat versi 0.1.0, edition 2024, rust-version 1.88. Fitur default mimalloc dapat dimatikan. Library tidak ditujukan sebagai API publik stabil; keberadaannya terutama membuka entrypoint bagi fuzz targets.

Dependency inti membentuk satu stack: Pingora untuk proxy/load-balancing/cache, OpenSSL vendored untuk TLS, Tokio untuk runtime/task, ArcSwap untuk snapshot, Serde + serde-saphyr untuk YAML, regex bytes/Aho-Corasick/memchr untuk matcher cepat, prometheus untuk metrik, instant-acme untuk protokol sertifikat, dan bcrypt untuk basic auth. Rustls-pki-types dipakai sebagai tipe data ACME; keberadaannya tidak berarti ada konektor TLS Rustls kedua.

Release memakai LTO fat, satu codegen unit dan strip symbols. Ini mengutamakan optimisasi akhir dan ukuran, dengan tradeoff waktu build/debug. Semua 15 patch crate Pingora menunjuk satu tree lokal. Konsistensi root manifest dan fuzz manifest penting: Cargo membaca patch dari root workspace aktif. Daftar patch keduanya selaras secara statis.

### Cargo.lock

Lockfile merekam graph resolusi dependency; ia bukan bukti bahwa source vendor sudah ada atau bebas advisory. Inventaris memuat hash. Untuk audit supply chain lanjutan, bandingkan gitlink fork, source crate registry, license dan advisory pada toolchain aktual. Jangan mengganti locked version berdasarkan dugaan atau menilai versi sebagai usang hanya dari tanggal pelatihan model.

### Dockerfile

Tahap build Ubuntu 22.04 bertujuan baseline glibc 2.35. Toolchain di-pin lewat ARG, OpenSSL dibangun dari source, llvm-tools dipasang untuk PGO, lalu source dan vendor disalin. Cache mount mempercepat registry/build. PGO default menjalankan workload sebelum build akhir. Tahap scratch adalah wadah artefak untuk `buildx --output`, bukan image service yang siap dieksekusi. Tidak ada CI build yang dapat dijalankan dalam audit ini.

### mise.toml

Menyatukan perintah test/lint, tes Docker ACME/CrowdSec, benchmark, compare, PGO dan release. `test` menjalankan `cargo test --all-targets`; ignored test tetap tidak berjalan. `lint` menjalankan format dan clippy dengan warning fatal. Task convenience membantu konsistensi lokal, tetapi belum membuktikan enforcement pada merge.

### File tersembunyi dan legal

- `.gitmodules`: menunjuk fork Pingora dan branch conduit; commit gitlink tetap sumber pin nyata.
- `.gitignore`: target/dist/local serta cache graph dikecualikan; graph.json/html/report sengaja dibagikan.
- `.graphifyignore`: mengeluarkan vendor, seed fuzz dan aset gambar. Konsekuensinya peta graph bukan representasi semua byte repository.
- `.dockerignore`: menghindari build artefak lokal dan metadata git ikut ke context; vendor source tetap diperlukan.
- `.gitattributes`: aturan repository, termasuk perilaku diff/berkas; inventaris merekam isinya untuk traceability.
- `LICENSE` dan `NOTICE`: lisensi Apache dan atribusi. Pemeriksaan legal dependency lengkap tidak dilakukan.
- `README.md`: ringkasan fitur, status, benchmark dan cara menjalankan; F10 membedakan salah dokumentasi dari implementasi.
- `conduit.example.yaml`: contoh konfigurasi menyeluruh dan petunjuk penggunaan. Contoh executable lewat test config; dalam audit ini test tersebut belum dijalankan.

## src/: 29 modul Rust

### 1. src/lib.rs — batas library

Seluruh file terdiri dari dokumentasi, lint allowance terbatas dan ekspor modul. `fuzzing` disembunyikan dari dokumentasi, tetapi tetap module publik agar workspace fuzz dapat mengakses fungsi pembungkus. Tidak ada database, migration, ORM, frontend atau persistence aplikasi lain yang perlu diasumsikan di luar struktur ini.

### 2. src/main.rs — proses, listener dan service lifecycle

**Baris 1–87:** import, allocator global opsional, socket options IPv6-only dan builder listener. Buffer write di listener membantu menggabungkan header/body kecil dalam satu flush; ini menukar latensi/memori per connection dengan syscall/packet overhead. IPv6-only memungkinkan `[::]` dan IPv4 listen berdampingan.

**Baris 89–178:** pilihan CLI run/check, prctl transparent huge pages dan dispatch. Check tidak cuma parse YAML: ia membangun runtime, membuka file dan me-resolve backend. Karena itu keberhasilan check bergantung pada environment saat dijalankan, tetapi tetap bukan test bind listener atau network health upstream.

**Baris 180–235:** startup mengaplikasikan cache config, compile runtime/cert, mengatur server header global, lalu membangun Shared. Config listen/admin/access_log path dipisahkan sebagai setting restart. `pool_per_worker` ditentukan saat start dan tidak mengikuti perubahan work_stealing lewat reload.

**Baris 237–297:** ServerConf, daemon readiness, bootstrap dependencies, HTTP/HTTPS listener, mTLS callback, admin service dan empat background services. Writer access log baru dimulai di service setelah fork—pilihan penting karena thread tidak survive fork. Shutdown flush dua detik adalah best effort, bukan fsync/durable logging guarantee.

Periksa default threads dua terhadap CPU deployment nyata, ukuran FD dan total connection buffers. Banyak fitur memakai Linux API langsung; portability bukan target implisit.

### 3. src/config.rs — bahasa konfigurasi dan deserialisasi

**Baris 1–96:** Config adalah plain data; `load` membaca file dan `yaml` memilih strict boolean, tanpa snippet rahasia di error. Unknown fields ditolak pada struktur. `or_default` menerima section null menjadi default. Error duplicate key mempertahankan posisi. Ini mengurangi typo silent dan paparan isi config di log.

**Baris 98–420:** schema server/listen/admin/defaults/access_log/real_ip/TLS/ACME/CrowdSec/default_site. Default body limit 2000 MiB cukup besar; ini limit streaming, bukan alokasi buffer otomatis sebesar itu. Body AppSec hanya sampai 64 KiB dan kegagalan default allow. Header Server custom di-validate lebih lanjut oleh runtime.

**Baris 427–819:** SiteConfig, mode upgrade, file server, cache site/global, mTLS, referensi upstream dan pool/health/passive schema. Deserializer cache menerima true/map/null tertentu dan sengaja menolak false dengan instruksi remove. Upstream string berisi `://` diperlakukan inline URL, lainnya nama pool; validasi resolve menolak referensi salah.

**Baris 820–1279:** plugin tagged maps, rate/concurrency/basic/forward/body_replace/exploit, matcher dan action. Representasi eksplisit mempersempit input konfigurasi dibanding parser bebas. Banyak validasi semantik terjadi di modul compile masing-masing; akibatnya perlu audit konsistensi batas numerik seperti F05/F06, bukan sekadar schema benar.

**Baris test berikutnya:** parsing minimal, plugin, example config, boolean/duplicate YAML dan ACME. Tes ini penting untuk kompatibilitas format, tetapi belum mencakup semua kombinasi runtime. Semantik empty `present` memang didokumentasikan, jadi tidak salah hanya karena beda dari contains_key.

### 4. src/runtime.rs — kompilasi konfigurasi menjadi snapshot

**Baris 27–93:** Site menyimpan pool/plugin/file/cache yang sudah compiled, timeout final, metrik handle dan mTLS trust. AcmePlan menyimpan storage, direktori CA dan site target. Request tidak mem-parsing config lagi.

**Baris 94–188:** namespace storage ACME menggunakan host directory URL dan hash FNV yang dipotong 32 bit; baik untuk nama stabil, tidak dimaksudkan sebagai hash keamanan. Host ACME di-sort/dedup, wildcard HTTP-01 ditolak dan stored PEM dicek sebelum dipakai. Sertifikat belum terbit menghasilkan warning, tidak menolak startup seluruh proxy.

**Baris 190–244:** Router exact memakai map; wildcard diurut suffix terpanjang. Lookup exact O(1) ekspektasian; wildcard O(jumlah wildcard) dengan perbandingan suffix. Banyak nested wildcard memerlukan benchmark realistis jika jumlahnya besar. Runtime mengikat resource request dan watched files.

**Baris 246–368:** validasi ID/host, resolusi pool reusable, compiled files/cache/plugin, trust CA dedup per path. ID tidak menerima slash, sehingga nama PEM berdasarkan site tidak dapat langsung menjadi traversal. CA trust id memisahkan koneksi sebelum/sesudah CA berubah. Validasi hostname bersifat praktis, bukan validasi DNS label penuh.

**Baris 371–548:** build cert global, pool named, plan, site enabled, duplicate IDs/hosts, unused-pool warnings, real IP dan default page. Host yang disabled tidak dimasukkan runtime. Sebagian resource/metrik dibuat sebelum validasi semua site selesai; publikasi snapshot ditunda sampai sukses tetapi registry global bisa telah berubah.

Runtime mempertahankan pool dengan signature sama; cache entry global tidak di-version bersama Site (F04). Baris komentar yang menyebut Runtime dan CertStore di-swap atomik perlu dipahami sebagai atomic store masing-masing, bukan transaksi dua pointer.

### 5. src/proxy.rs — koordinator utama request

**Baris 40–115:** Shared adalah state proses; Ctx adalah state request. Memisahkan keduanya mencegah kontaminasi antarrequest dan mempermudah lifecycle. Ctx memegang runtime/site snapshot, lease attempt, failed backends, byte counters, plugin effects, rewrite streams, auth edits dan hasil cache.

**Baris 133–214:** Ctx baru menaikkan in-flight dan Drop menurunkannya. UpgradeLog menyimpan data tambahan hanya saat upgrade aktif; Drop menghasilkan log Cancelled jika runtime memutus koneksi sebelum logging hook. Biaya alokasi tambahan dibatasi koneksi upgraded, bukan semua request.

**Baris 216–386:** static parsed header names mengurangi parsing/alokasi. Request ID berisi prefix process dan counter yang dibagikan perthread dalam blok 4096; ini ID korelasi, bukan token autentikasi atau urutan waktu global. Host parser dipakai routing dan key; penggunaan untuk cache memunculkan F03. URI raw dipertahankan agar forwarding tidak diam-diam mengubah target.

**Baris 388–480:** scheme forwarding memercayai hanya peer trusted, mTLS menolak trust berbeda dengan 421 dan certificate missing/invalid sesuai optional. Header identitas sertifikat dibersihkan sebelum diset dari connection. Plugin header operator masih bisa menimpanya belakangan; itu kewenangan konfigurasi, bukan input client langsung.

**Baris 482–624:** route pool dan routed_target, local decision ACME/default/HTTPS/mTLS/body limit serta plugin chain. ACME token diperlakukan khusus sebelum proteksi site, sesuai kebutuhan validasi CA. `run_plugins` normalisasi path hanya jika chain membutuhkan. Normalizer path bukan universal canonicalization untuk seluruh semantik framework origin.

**Baris 627–752:** AppSec membaca body kecil ke retry buffer lalu inspect; file/cache decision datang setelah plugin. File matcher memakai normalized path. Route proxy yang sudah dipilih menonaktifkan file serving. File miss pass-through tidak otomatis menjadi cache eligible karena cabang file lookup telah mengembalikan false; ini pilihan coverage cache yang perlu dipahami saat mengukur hit rate.

**Baris 754–864:** status page, decorate, respond dan compression MIME. Respons lokal menutup reuse bila request body belum dibaca. HEAD tidak menulis body. Plugin response berjalan untuk jawaban lokal via decorate. Status 301 default HTTPS mempunyai implikasi perubahan method client seperti umumnya redirect 301; bukan 308 yang menjamin mempertahankan method.

**Baris 876–983:** hook request_filter memuat Arc penuh sekali, menetapkan client keepalive, resolve RealIP, menjalankan decision lalu async auth berurutan. Setelah check sukses, chain dilanjutkan dari index berikutnya. `upstream_edits` tidak mengubah request asli yang dilihat matcher setelah auth. Ctx diisi sebelum AppSec/file/network phase.

**Baris 985–1115:** body size diperiksa setiap chunk; sesudah 101 bytes adalah protocol stream dan tidak dibatasi sebagai request body. Pool memilih backend dan lease, timeout/site diterapkan ke peer, group perworker memisahkan connection pooling. Connect failure mencoba backend lain sesuai retry budget, aman pada jalur ini karena request belum diterima backend. Retry framework lain harus ditinjau di vendor.

**Baris 1117–1272:** upgrade hop headers, XFF chain, X-Real-IP, original Host, trusted forwarded scheme/host, request ID, mTLS/auth/plugin header edits, routed target dan strip Accept-Encoding/Range untuk body replacement. Urutan edit menentukan representasi yang benar-benar diminta origin—cache key harus mencerminkannya.

**Baris 1274–1339:** response hooks menerapkan decoration, memilih rewrite stream menurut content type, Cache-Status serta MIME/status compression gating. Rewrite mengubah byte setelah cache source tersimpan; entry menyimpan respons origin, sehingga aturan baru dapat diterapkan pada hit. Namun perubahan request-side headers yang memengaruhi origin tetap butuh namespace invalidation.

**Baris 1343–1433:** enable cache manager/storage/predictor/lock, cache key identity, response cacheability, Vary dan stale decision. Auth identity serta certificate fingerprint sudah diikutkan—jangan menyimpulkan semua pengguna auth berbagi body tanpa memeriksa field ini. F03/F04/F09 merupakan gap yang lebih spesifik.

**Baris 1435–1574:** fail_to_proxy memilih status, berusaha mengirim error hanya jika response belum mulai, melarang reuse downstream pada error. Logging menghitung response/site/cache, menghitung passive failure tertentu dan mengabaikan background subrequest sebagai request client. 499 adalah status observasi ketika client pergi, bukan response yang pasti dikirim.

**Baris 1576–2033:** unit tests decision, URI/routing/ID/compression/mTLS. Test ini memeriksa pure path tertentu; e2e diperlukan untuk hook order framework, streaming dan timeout.

### 6. src/plugin.rs — kontrak dan urutan policy

`Local` memodelkan respons lokal. `Gate` bisa pass/answer/check/route/replace. `Effects` menyimpan permit concurrency, route pertama dan seluruh replacement berurutan. `CheckRequest` membawa scheme forwarding dan resolved client; `UpstreamEdits` memodelkan set maupun remove header.

`Chain::build` (sekitar baris 180–256) mengompilasi plugin sesuai urutan YAML, memberi pool inline nama deterministik berdasarkan site dan indeks rule, serta menggabungkan flags needs_path/appsec. Tidak ada dynamic plugin loader atau eksekusi kode arbitrer dari YAML.

`Chain::run` (baris 282–297) berhenti pada answer/check. Route tidak menghentikan chain; route pertama menang. `upstream_request` dan `response` menjalankan hook seluruh plugin sesuai urutan; hook header tidak bergantung pada apakah gate plugin tersebut pernah dilewati request. Ini sesuai desain HeadersPlugin tanpa matcher.

RulesPlugin mengambil rule pertama yang match, body_replace hanya menambahkan effect, HeadersPlugin menerapkan remove sebelum set, IpFilter memeriksa deny/allow atas real client address. Susunan rule respond sebelum basic_auth dapat sengaja membuat endpoint public; itu kontrak urutan yang perlu direview di config site, bukan bug engine secara umum.

### 7. src/rules.rs — matcher, template dan action

BytesMatch memakai exact/prefix atau regex bytes. Prefix tidak menyaratkan batas segmen; `/admin` juga cocok `/administrator`. Ini aturan string, bukan router segmen. Regex dikompilasi saat build, tidak pada setiap request.

HeaderCond memaksa tepat satu dari present/equals/regex. Beberapa header disatukan dengan koma sebelum match. Header yang secara protokol tidak comma-combinable tetap perlu konfigurasi yang sadar kontrak ini. Present berarti value tidak kosong, cocok dokumentasi.

Matcher meng-AND seluruh jenis kondisi dan OR daftar method. Method config di-uppercase; method request dibandingkan sebagai Method. Query dan URI tidak di-decode; path sudah normalized oleh caller. Perbedaan ini mencegah ambigu tetapi harus dipahami saat menulis regex.

Template mengizinkan hanya scheme/host/path/request_uri. Nilai render dimasukkan ke HeaderValue yang menolak control invalid, mengurangi CRLF injection. Placeholder path yang decoded dapat berisi karakter URL khusus; untuk redirect presisi operator sebaiknya memahami encoding atau memakai request_uri.

Action compile memvalidasi status, header dan route; action file membaca file penuh saat build lalu watch path. Ini berbeda dari static file streaming: isi rule-file tinggal di memory snapshot dan dibaca ulang saat reload. File besar di action config menambah memori runtime lama dan baru saat reload.

### 8. src/route.rs — rewrite target origin

Route menyimpan optional pool dan prefix/regex rewrite. Query dan fragment dilarang pada prefix/template rewrite config; raw query client dipertahankan terpisah. Regex replacement dapat menggunakan capture.

Path rewrite bekerja pada normalized decoded path, kemudian encode_path meng-escape persen dan byte di luar karakter path. Ini menjaga literal `?`/`#` agar tidak berubah menjadi query/fragment. Strip prefix bersifat string dan `/api` dapat memotong `/apix`; test secara eksplisit memilih perilaku itu.

Perlu membedakan rewrite target yang valid secara syntax dari target yang memenuhi policy aplikasi origin. Fuzz route membuktikan pembentukan origin-form dan preservasi query, bukan izin resource origin.

### 9. src/pathnorm.rs — canonicalization terbatas

`hex` membaca digit escape, `percent_decode` strict menolak escape tidak lengkap/nonhex dan NUL, lalu normalize memecah slash, menghapus dot dan pop dotdot. Dotdot di atas root adalah error. Trailing slash dipertahankan secara eksplisit.

Output belum tentu idempotent terhadap decoding karena `%25` menjadi `%`, yang bisa memulai escape pada pemanggilan kedua. Normalizer ini benar sebagai transformasi satu kali terhadap raw bytes; F01 muncul karena caller memperlakukannya seperti normalisasi segmen yang boleh diulang. Pisahkan kedua operasi agar invariannya dapat diuji lintas modul.

### 10. src/realip.rs — batas kepercayaan alamat client

Cidr parse memvalidasi family/prefix dan containment memakai mask dengan /0 aman. IPv4-mapped IPv6 dikonversi agar filter IPv4 tidak dilewati via bentuk mapped. Daftar Cloudflare dibundel sebagai data; audit ini tidak memverifikasi perubahan rentang terkini terhadap endpoint provider.

RealIp.resolve mengabaikan header jika socket peer tidak trusted. Recursive mode berjalan dari kanan dan berhenti pada hop untrusted atau token malformed. Karena caller memberi satu HeaderValue melalui `get`, konfigurasi XFF perlu tes repeated header untuk konsistensi dengan framework origin; tidak boleh menyimpulkan chain semua field pasti disatukan di resolver ini.

Pilihan trusted CIDR merupakan security-sensitive operator input. Memercayai 0.0.0.0/0 membuat client dapat menyatakan IP; parser tidak dapat menyelamatkan policy trust yang terlalu luas.

### 11. src/auth.rs — basic dan forward authentication

BasicAuth (baris 70–283) mem-parse credential Basic, memvalidasi bcrypt hash saat compile dan menyimpan cache bounded dari digest ber-key proses atas credential yang sukses. Password plaintext tidak disimpan di verified set. Pengguna tak dikenal tetap diverifikasi terhadap decoy hash agar tidak menjadi jalur murah yang jelas.

Bcrypt berjalan dalam spawn_blocking. Semaphore global membatasi parallel hash per jumlah CPU; permit dipindahkan ke closure sehingga client cancellation tidak melepas kuota sebelum kerja CPU selesai. Timeout lima detik membatasi waktu menunggu permit, bukan durasi hash yang sudah mulai. Hash cost sangat besar dari operator dapat tetap menyita CPU lama; tidak ada timeout yang membatalkan bcrypt running.

Verified cache maksimum 1024 lalu clear seluruhnya: sederhana/bounded, tetapi eviction storm pada banyak user dapat menaikkan biaya hash. Cache dibangun ulang saat reload, sehingga revokasi hash efektif untuk request baru pada snapshot baru; request lama tetap memakai snapshot lamanya.

ForwardAuth (baris 285–456) membuat GET tanpa body ke auth URL, menghilangkan forwarding headers client lalu mengisinya dari context. Total timeout membungkus DNS/connector/body. Respons 2xx mengambil copy_headers; header yang tidak ada berubah menjadi remove, mencegah client menanam identity header yang auth tidak berikan. Non-2xx kembali sebagai jawaban lokal, dengan header hop/framing disaring.

copy_headers hanya mengambil satu nilai per nama; cocok identity sederhana tetapi jangan berasumsi header multivalue selalu diteruskan utuh. Header request ke layanan auth tidak dibatasi allowlist, jadi layanan auth menerima banyak metadata request yang mungkin sensitif; itu kontrak integrasi yang perlu disadari operator.

### 12. src/limits.rs — admission dan state lintas reload

Key client memampatkan IPv6 ke /64, host key shared, header key fallback client jika missing. Hasher dipertahankan bersama sketch agar key tetap sama saat reload. Header key berasal dari request client asli, bukan hasil auth edits yang belum diaplikasikan.

CARRIED berisi Weak state menurut jenis/site/index/settings. Perubahan posisi plugin menyebabkan state baru walaupun settings sama. Weak map dibersihkan saat ada pembangunan limiter berikutnya, bukan timer; ini bukan kebocoran state kuat permanen.

RateLimit memakai count-min sliding-window weighted count, fixed memory. F07 menunjukkan keputusan dan observasi belum satu admission atomik. Concurrency memakai incr lalu cek/decr dan menyimpan Held yang decrements pada Drop; lebih cocok membatasi request yang benar-benar sedang hidup. Sketch collision dapat menolak client innocent; ini tradeoff memory/cardinality, bukan limiter eksak perclient.

### 13. src/files.rs — filesystem, negosiasi dan streaming

**Baris 39–140:** tiga encoding precompressed serta bitset Accepted; mengenali q=0 secara ringan, bukan parser penuh weighted negotiation. F09 muncul saat parser untuk memilih sibling juga dipakai mengkanonisasi seluruh Vary HTTP.

**Baris 143–305:** Candidate membagi literal dan placeholder path; Hide membatasi pola nama. Compile memeriksa root directory, index plain name, strip prefix dan hubungan pass_thru/matcher dengan keberadaan upstream. Root sebagai path belum dipin dengan FD sepanjang lifetime; root dapat diredeploy lewat symlink oleh operator.

**Baris 307–445:** find mengaplikasikan strip prefix dengan batas segmen, candidate/try_files, hide, open/stat, redirect slash directory, index, browse, precompressed sibling dan lookup spawn_blocking. Dotfiles kecuali .well-known disembunyikan secara lexical. F01 ada pada normalisasi candidate setelah expansion; F02 pada fallback open dan reopening listing.

**Baris 450–641:** data Lookup/FileReply, metadata ETag dari nanosecond mtime+length, read_exact_at untuk file kecil dan open_beneath. ETag ini bukan content hash; pergantian isi dengan mtime dan length identik dapat mempertahankan validator—pertimbangkan kontrak deployment file. read_at memakai offset independen sehingga Arc<File> tidak berbagi cursor mutable antar task.

**Baris 643–827:** MIME table, conditional ETag/date dan single range parsing. Multi-range diabaikan menjadi full response, bukan multipart. Range malformed juga diabaikan; unsatisfiable menghasilkan 416. Urutan precondition mengutamakan If-Match/If-None-Match atas tanggal. Strong/weak ETag semantics perlu dipertahankan saat refactor.

**Baris 829–911:** send menyusun status/header, HEAD/304/bodyless, precompressed/range, opsi compression dan stream file besar per 128 KiB via blocking pool. Backpressure mengikuti write ke Session. File yang menyusut saat sedang dibaca memicu error, tidak menambah byte palsu sampai declared length.

**Baris 913–1002:** HTML/url escaping dan directory listing dibatasi 10 ribu entri. Batas jumlah entri tidak berarti output selalu kecil karena nama file bisa panjang. Metadata symlink listing memakai std::fs::metadata; jangan samakan dengan pembukaan file bersyarat openat2.

**Test berikutnya:** symlink containment biasa, file/index/try_files/encoding/range/precondition; race filesystem perlu test lain yang memaksa interleaving.

### 14. src/cache.rs — shared HTTP cache adapter

SiteCache.compile membentuk defaults TTL/stale. takes membatasi GET/HEAD tanpa Upgrade atau body; response cacheable menolak Set-Cookie, 206 dan Vary *. Authorization diberitahukan ke filter framework; default_ttl fallback dibatasi agar tidak sembarang meng-cache respons authenticated. Implementasi detail shared-cache RFC berasal dari fork dan belum dapat diverifikasi.

variance (baris 139–182) mengumpulkan semua Vary names, menyatukan repeated values dan menambah Accept-Encoding jika body encoded. Framing key dengan panjang per part menghindari concatenation ambiguity seperti `ab|c` lawan `a|bc`. F09 pada normalisasi encoding adalah pengecualian terhadap sifat key yang otherwise hati-hati.

Store (baris 278–405) mempunyai 16 shard Mutex map, Arc<Object>, serialized metadata, Bytes body dan identity entry ID. Counter byte mencakup body, serialized header dan overhead estimasi; bukan batas RSS proses secara keras. Request hit masih memegang Arc body setelah eviction; miss buffering dan TLS/connection buffers berada di luar eviction capacity.

Storage trait (baris 407–510) mendukung lookup, miss, purge exact/active, expire, metadata rewrite. Exact purge memeriksa entry ID agar eviction lama tidak menghapus replacement baru. Body yang diganti metadata dibagi dengan Bytes, menghindari copy besar.

Hit/Miss (baris 512–610) membaca Bytes slice, seek untuk ranges dan menampung miss body sampai finish. Batas max_object_size diserahkan ke Session.cache, bukan checked setiap write di Miss. Perlu vendor test untuk memastikan framework menghentikan miss yang melampaui ukuran, termasuk chunked tanpa length. Jangan menyatakan max_size menjamin tidak pernah ada transient overcommit.

Purge berdasarkan prefix memakai raw URI (termasuk query). Admin unescape query purge tidak membuatnya menjadi normalized-path purge. In-flight insert sesudah purge dan konfigurasi baru memerlukan generation jika invalidasi harus kuat (F04).

### 15. src/upstream.rs — discovery, pemilihan dan health

Target menampung resolved socket, TLS/SNI/verify, weight, counters dan ejection clock. Lease menaikkan request/inflight saat select dan menurunkan inflight pada Drop; requests di sini menghitung pemilihan attempt, bukan hanya respons sukses.

Resolve (baris 193–261) menerima scheme http/https, authority tanpa path, bobot positif, me-resolve DNS saat build dan menolak duplicate socket address. Nama DNS dengan perubahan IP baru dipakai saat reload; tidak ada refresh TTL periodik independen. Menyebutnya dynamic discovery tanpa penjelasan ini menyesatkan.

Pool::build (baris 264–344) memakai signature resolved/config untuk reuse Arc lama; health/ejection/retries carry over saat identik. Health rise/fall diperiksa positif, interval/timeout belum seluruhnya diberi domain validasi. Least_conn weighted ratio memakai multiplication usize; bobot operator ekstrem perlu checked domain jika ingin bebas overflow.

select/choose/least_conn (baris 390–438) memprioritaskan healthy/non-ejected, lalu fail-open any. Scan selection dibatasi 256 candidate. Least_conn O(n) dan ties dirotasi; satu backend punya fast path. Least_conn berarti in-flight request, bukan pengamatan socket TCP terbuka.

Passive failure menghitung connect/error sebelum response header tertentu; 5xx dari aplikasi memanggil report_success karena response header sudah diterima. Active probe memberi arti sehat sesuai status expect, default 2xx/3xx, sehingga redirect login pun bisa dianggap sehat. Operator dapat mengatur expect lebih ketat.

HealthService tick 250 ms memulai probe due dan flag probing mencegah overlap pool. Inner probe dibatasi timeout total dua kali. Pool lama yang masih dipegang request tetap hidup sementara, tetapi hanya pool runtime terbaru yang dijadwalkan berikutnya. Audit cancellation flag perlu mempertimbangkan task panic—normal Result error tidak menahan flag.

### 16. src/tls.rs — material sertifikat dan identitas mTLS

CertKey.load (baris 42–92) membaca leaf+chain dan private key, membandingkan public key, mengambil SAN atau fallback CN, menghitung expiry dan warning expired. Ia bukan verifikasi chain server lengkap atau proof bahwa semua configured hostname tercakup. Expired cert boleh tetap dimuat; client menolaknya saat handshake.

CertStore (baris 95–211) mengelola name/exact/wildcard/default/override. SAN overlapping memakai first insert wins. Wildcard cert hanya satu label; site override wildcard belum ditelusuri dengan benar (F08). Sertifikat default membantu SNI tak dikenal tetapi routing HTTP tetap terpisah.

ClientTrust.load menghash PEM CA sebagai trust ID. Ini stabil selama byte file sama; perubahan whitespace/reorder PEM pun menghasilkan trust baru walau trust semantik sama, aman konservatif tetapi memicu rehandshake.

ClientCert.read menghasilkan Missing/Valid/Invalid dari digest handshake. Fingerprint SHA-256 dan DN escaped menjadi identity headers; DN formatting bukan verifikasi otorisasi subject aplikasi. Raw unsafe FFI dibatasi pembacaan ClientHello extension dan setting session context. Lifetime slice hasil OpenSSL disalin sebelum callback selesai.

ask_for_certificate memasang store, CA list, session ID context dan verify callback yang selalu melanjutkan handshake; status hasil verifikasi dinilai di request untuk response 403/421. Melanjutkan handshake bukan bukti semua client cert diterima sebagai authenticated. Perlu tes invalid cert, resumed sessions dan CA rotation—suite memang punya mTLS scenario, belum dijalankan di audit ini.

### 17. src/acme.rs — penerbitan, persistence dan renewal

CHALLENGES process-global menyimpan token sementara. Published RAII menghapus token ketika order selesai atau dibatalkan. Token lookup boleh dijawab pada host mana pun; entropy token/protokol CA menjaga otorisasi domain, bukan router lokal semata.

Adapter Http membawa request instant-acme ke httpc/OpenSSL sehingga tidak menambahkan transport stack kedua. Max response satu MiB. Body respons lengkap dibuffer karena JSON/PEM kecil; timeout per operation tersedia, total deadline transport ACME belum seseragam forward-auth.

write_private menulis file mode 0600 ke temporary nama per PID, fsync file, rename dan best-effort fsync directory. Ini menghindari key/cert terpisah saat renewal. Direktori dibuat 0700 bila baru. Threat actor yang sudah dapat menulis private storage dapat mengganggu temp path; containment storage mengandalkan permission OS.

try_lock menggunakan flock nonblocking agar old/new process saat graceful upgrade tidak menerbitkan bersamaan. Mutex in-process bukan pengganti file lock antarprocess; di sini keduanya memiliki kegunaan berbeda.

Order flow: names → new order (opsional replaces) → authorization HTTP-01 → publish token → set ready → poll ready → CSR/key baru → finalize → poll certificate → bundle. Token dibuang sebelum finalize ketika authorization sudah selesai. Bundle memastikan key dan names cocok sebelum disimpan.

Manager menentukan due dari names/expiry/ARI, dengan lifetime fallback dan jitter. Failures memakai exponential backoff lima menit sampai enam jam. CA ARI error memakai cache fallback; status exposes last_error/retry. Sites diproses berurutan, jadi CA lambat pada satu site dapat menunda yang lain. Perlu workload banyak site untuk menilai waktu issuance keseluruhan.

Account memoization berdasar directory dan CA path, bukan storage atau email. Perubahan kontak/storage saat reload perlu kontrak eksplisit. Service wake dipicu reload agar site baru tidak menunggu satu jam; shutdown select membatalkan future, Published menjamin token cleanup.

### 18. src/crowdsec.rs — local decisions dan AppSec

Family/Table (baris 46–344) menyimpan exact IP dan range perprefix, tidak mengembangkan /16 menjadi puluhan ribu IP. Decision pernet bisa banyak ID; remedy maksimum Ban diprioritaskan. Expiry memakai monotonic clock, menghindari perubahan jam dinding. IPv4 mapped dikonversi pada parsing dan lookup.

go_duration_ms (baris 346–386) membaca format durasi Go termasuk pecahan. Penggunaan f64 dan saturating cast tidak sama dengan parser integer eksak; response LAPI ekstrem perlu boundary tests. Data berasal dependency LAPI terpercaya, bukan input HTTP client umum.

DeserializeSeed/Visitor (baris 389–558) membaca wire response dan mengumpulkan full table atau delta. Full list dibangun terpisah lalu dipublish; delta diparse dulu sebelum lock write. JSON malformed tidak menerapkan separuh perubahan pada store hidup. Ini keputusan transaksi yang baik.

Plan::build (baris 652–757) memaksa tepat satu sumber API key, menandai HeaderValue sensitive, compile query filter, interval minimum dan body limit. Key file/ban page masuk watcher. Source signature berubah saat key/filter/url/CA path berubah, memaksa full pull.

Pull/sync/service (baris 788–926) memakai timeout keseluruhan, resync full setelah failure, memelihara keputusan lama sampai expire dan prune berkala. `RwLock` tetap shared request-path; delta besar memblokir reader selama apply walau parsing di luar lock. Ukur p99 saat full list/delta besar sebelum mengganti struktur.

AppSec (baris 934–1101) memisahkan transport identity x-crowdsec-appsec-* dari headers client, mengirim GET untuk body kosong/POST untuk body inspected, membatasi waktu dan respons. Status 200 allow, 403 block dengan http_status yang disaring 4xx/5xx; error mengikuti policy failure_action. Body chunked/Expect/terlalu besar tidak dibuffer. Request body protection bukan WAF universal.

Plugin CrowdSec lokal memeriksa resolved client sebelum cache/file/upstream; keputusan baru tidak dilewati hanya karena body sudah cached. Startup table bisa kosong sebelum pull pertama; fail-open cold-start perlu dipahami operator jika menuntut enforcement sejak request pertama.

### 19. src/exploits.rs — signature engine

Category dan Def (baris 39–154) memisahkan ID/kategori/input scope/needle/regex/verifier. Mask u64 memberi operasi cepat tetapi membatasi 64 rules. Semua compile sekali lewat LazyLock; panic compile signature muncul pertama kali engine diinisialisasi jika pattern invalid, bukan parse request tanpa batas.

Tabel defs (baris 315–725) mencakup JNDI/Shellshock/OGNL/Spring/SSTI/Next.js subrequest, traversal, secret files, PHP wrappers/code, SQLi, XSS, command/metadata SSRF, scanner dan spam. Scope perrule penting: beberapa path repo sengaja diperbolehkan agar tidak banyak false positive. Ini signature deny heuristik; tidak membuktikan aplikasi di belakang kebal kategori exploit tersebut.

Engine (baris 786–1036) memetakan Aho-Corasick needle ke candidate bitmask, kemudian regex spesifik dan verifier. Lowercase dibuat sekali per input bila diperlukan. Query di-decode sekali dan opsional dua kali, `+` menjadi spasi, skip_params dipisahkan sebelum scan. SQL comment stripping menambah varian. Header hanya discan sesuai prefilter; payload di body memerlukan AppSec.

fuzz_check membandingkan fast scanner dengan scanner lambat, sehingga menjaga needle tidak menyembunyikan regex match. Itu oracle internal yang kuat untuk optimisasi, tetapi kedua scanner menggunakan signature sama sehingga tidak membuktikan tidak ada bypass signature. Korpus benign/replay yang disebut README membantu false-positive assessment, tetapi dataset tersebut tidak diaudit ulang.

Scratch TLS buffer dibersihkan bila kapasitas lebih dari 64 KiB untuk menghindari retention request sangat besar. Logging detection dibatasi laju, URI dipotong dan escape_debug agar control bytes tidak mengacaukan log. Metrics label kategori fixed, tidak memakai URI client sebagai label.

### 20. src/replace.rs — transformasi body streaming

BodyReplace mengompilasi Aho-Corasick LeftmostFirst sesuai urutan rule, melarang pola kosong dan memilih content types. Stream menahan paling banyak longest-from minus satu byte di antara chunk; match yang belum pasti ditunda. Output tiap replacer mengalir ke replacer berikutnya sesuai chain.

rewritable melewati HEAD, bodyless, 206, 304 dan compressed nonidentity. rewritten_headers menghapus length/ranges/validator yang merujuk byte origin dan menambah chunked untuk HTTP/1.1. Penggantian dapat memperbesar output secara besar walaupun carry bounded; memory total chunk/output tidak hanya carry.

Tests mencoba dua cut di semua posisi serta satu byte perchunk. Ini properti algoritme penting. Digest respons seperti Content-MD5/Content-Digest, bila origin mengirimnya, juga dapat menjadi usang setelah transformasi dan perlu keputusan filter eksplisit; belum dijadikan temuan final karena kontrak respons/digest perlu tes tersendiri.

### 21. src/httpc.rs — client internal

Client::resolve mendukung literal IP/DNS dan scheme http/https; authority port/SNI dibentuk dari Uri. send mencoba beberapa address serta retry koneksi pooled yang tampak stale. Retry classification menganggap kegagalan sebelum response header sebagai kandidat stale; untuk request nonidempotent ACME, ambigu apabila server memproses request lalu koneksi putus sebelum header. Protokol ACME memiliki nonce/order semantics; perlu tes kegagalan sebelum memperluas guarantee retry-safe.

exchange menetapkan session read/write timeout secara eksplisit, menyusun Host/Content-Length, membatasi response body dan me-release keepalive setelah selesai. Max body check mencegah buffer tumbuh tanpa batas dari dependency, meskipun deadline total ditetapkan caller. Forward-auth/CrowdSec menambahkan timeout total; ACME adapter belum.

Source tidak memakai Reqwest/Hyper sebagai transport kedua. Reuse setting internal connector berbeda dari pool traffic upstream aplikasi, jadi tuning upstream_keepalive tidak otomatis men-tune semua internal call.

### 22. src/reload.rs — perubahan file dan publikasi state

Stamp terdiri mtime, inode dan size, mengikuti symlink target. Snapshot diambil sebelum read agar pergantian file saat compile terdeteksi tick berikutnya untuk path yang sudah watched. File baru ditambahkan punya timing berbeda dan perlu race test bila deployment sangat aktif. Mengubah isi dengan mtime/size/inode identik dapat tidak terdeteksi; SIGHUP tetap memaksa.

reload membangun runtime baru dan mempertahankan yang lama saat Result error. Setting listen/admin/log path diberi warning restart; seluruh server section dan watcher interval juga bersifat startup menurut docs, namun tidak semua perubahan diberi warning. Pool reused dapat dihitung di log.

Cache apply/retain dan cert/runtime store berlangsung berurutan, bukan transaksi global. F04 mengenai entry cache; F06 mengenai interval startup. Jika build panic akibat overflow, mekanisme Err untuk menjaga runtime lama tidak otomatis menanganinya.

Background loop menangani HUP/ACME wake/tick/shutdown. Notify menggabungkan wake, sesuai kebutuhan membaca state terbaru; tidak perlu satu reload per notifikasi jika banyak sertifikat terbit cepat. Interval Delay mencegah burst catch-up sesudah kerja lambat.

### 23. src/admin.rs — observability dan purge

Listener runtime satu thread, sengaja tanpa auth. `/healthz` adalah liveness statis, bukan readiness upstream atau kepastian ACME/bouncer sudah sinkron. `/status` mengungkap topology backend, path file, expiry dan last_error; loopback/management network adalah trust boundary nyata.

`POST /cache/purge` membaca site/prefix query dan menutup keepalive karena body diabaikan; GET/HEAD dipakai endpoint status/metrics. Unknown/malformed query ditolak, prefix harus slash, site harus masih caching. Repeated parameter last wins; tentukan kebutuhan strict duplicate rejection jika API dipakai banyak tool.

Status membaca runtime dan cert pointer terpisah sehingga snapshot observasi dapat lintas generasi pada reload. Ini lebih merupakan akurasi snapshot daripada bukti request policy bypass.

### 24. src/metrics.rs — metrik dan cardinality

Lazy metrics diletakkan pada default registry bersama Pingora. SiteMetrics menyimpan handle agar request tidak melakukan lookup label berulang. Label site/backend berasal config, kategori/status class finite; tidak memasukkan raw Host/URI sehingga tahan cardinality dari request random.

forget_site menghapus series site removed. Gauge backend/cert di-reset dan diisi saat scrape agar tidak tertinggal. Counter backend historis tetap ada dan gagal build dapat mendaftarkan label, sehingga churn konfigurasi dapat menambah memori/cardinality.

ProcessCollector diinisialisasi ulang saat scrape pertama setelah daemon fork agar PID yang dibaca benar. Ini detail operasional penting; sekadar membuat collector sebelum fork menghasilkan metrik parent yang sudah exit.

### 25. src/accesslog.rs — serialization cepat dan writer asinkron

Entry + Serialize derive adalah definisi schema. write_json manual menghindari overhead generic, dengan escaping delapan byte, integer formatting dan duration microsecond precision. Differential tests/fuzz terhadap serde_json sangat penting karena kode hand-written mudah meleset pada control bytes/UTF-8.

Shard log perthread membatasi lock contention. Buffer dibatasi jumlah lines perthread, minimum 1024; actual byte memory bergantung URI/header panjang dan jumlah thread yang menulis. Writer setiap 20 ms swap buffer agar critical section singkat, append ke BufWriter dan flush setiap detik. Rollover dicek lima detik dengan inode.

Ketika queue penuh, line drop dihitung. Open/write/flush disk error dilog tetapi data bukan durable retry queue; flush ack tidak berarti fsync disk atau semua write sukses. Dokumentasikan ini jika log digunakan untuk auditing wajib. Ordering perworker dalam jendela polling tidak sama dengan urutan completion global; timestamp membantu rekonstruksi.

### 26. src/logger.rs — error log

Logger membaca CONDUIT_LOG dan menulis stderr dengan UTC timestamp. Kasus reuse-stream race dan pesan compression dictionary diturunkan agar tidak membanjiri log; target dan teks dipersempit sehingga error lain tidak ikut hilang. Counter reuse_lost tetap mencatat frekuensi.

stderr write masih synchronous; flood warning/error dapat memberi backpressure. Exploit/auth warning punya rate limit terpisah. Config error mungkin memuat beberapa nilai operator, walau snippet YAML dimatikan; jangan menganggap seluruh error otomatis bebas secret tanpa memeriksa formatter modul.

### 27. src/timefmt.rs — waktu tanpa date library

civil_from_days mengubah epoch days dengan algoritme Gregorian. seconds_part menyusun UTC date/time, TLS cache menyimpan bagian detik untuk access log dan menambahkan millis numerik. SystemTime sebelum epoch dipetakan nol. Known instant/leap-day tests tersedia. Tanggal ekstrem di luar kebutuhan produksi belum dibuktikan dengan property tests.

### 28. src/units.rs — tipe ukuran dan durasi

Raw menerima angka u64 atau string. Size mengartikan 0 sebagai unlimited, mengalikan dengan checked_mul dan memakai KiB/MiB/GiB. Dur memakai Duration dan menerima ms/s/m/h/d, tetapi multiplication belum checked (F05). Nol bermakna valid pada parser umum; validator field harus menentukan apakah nol dilarang (F06).

Pemisahan parser umum dan domain validation adalah desain baik jika lengkap. Jangan melarang nol secara global karena beberapa pengaturan mungkin mengizinkan disable/immediate; tolak tepat pada konsumen yang membutuhkan periode positif.

### 29. src/fuzzing.rs — entrypoint properti

Config fuzz mem-parse lalu build hanya input tertentu untuk menghindari file khusus/DNS. Pemeriksaan backend inline di proxy rules belum sesederhana pool/site biasa; fuzz process tetap sebaiknya sandbox. Path fuzz memverifikasi absolute/no NUL/no dot segments. Route fuzz memeriksa encoding/query dan penerimaan target oleh Pingora.

Body_replace fuzz membandingkan whole/chunked hasil; access_log fuzz membandingkan serde dan menolak newline literal; exploit fuzz fast/slow; request fuzz menyusun request header/method/version/peer, menjalankan decision dan parser helper. Ia berhenti pada Step::Check dan tidak benar-benar menjalankan service auth. Karena itu bukan full-stack request fuzzer.

Fungsi wrapper dikompilasi ke library; binary linker dapat membuang symbol unused. Tidak ada endpoint HTTP fuzzing di production hanya karena modul publik.

## tests/: skenario binary nyata

`tests/e2e.rs` 5.160 baris menggabungkan helper port/socket/TLS/cert/process dan 20 test besar. Inventaris mencatat seluruh deklarasi. Pembagian domain yang ada:

| Test | Hal yang diperiksa | Batas bukti |
|---|---|---|
| end_to_end | Routing, headers, TLS, metrics, reload dan local policy | Satu kumpulan konfigurasi, bukan semua kombinasi |
| graceful_upgrade | Handover listener dan daemon lifecycle | Sangat tergantung patch fork |
| rotation_and_reload_under_load | Cert/config berubah sambil ada traffic | Tidak otomatis semua interleaving terbukti |
| acme_with_pebble | Issuance/renewal memakai CA test | Ignored, perlu Docker |
| crowdsec_bouncer_and_appsec | Bouncer, inspector dan parser log nyata | Ignored, perlu Docker |
| rate_and_concurrency_limits | Perkey dan lifetime request permit | Tambah barrier multithread F07 |
| mutual_tls | CA/client cert, optional, koneksi reuse | Butuh vendor/OpenSSL yang cocok |
| auth_plugins | Basic/forward auth dan identity headers | Tambah interaksi file decode/cache generation |
| file_server | File/encoding/range/security biasa | Tambah race forced-fallback dan double encoding |
| response_cache | Hit/miss/stale/purge/identity | Tambah authority port dan unknown encoding |
| protocol_upgrades | WebSocket/arbitrary upgrades dan policy | Tidak membuktikan protokol upgrade itu sendiri |
| request_and_response_bodies | Streaming, framing, batas dan timeout | Framework framing masih bagian vendor |
| upstream_flapping | Backend berubah sehat/gagal | Coverage failure schedule tertentu |
| upgrade_with_open_websockets | Koneksi upgraded saat binary diganti | Grace deadline bisa memutus yang terlalu lama |
| idle_and_malformed_clients | Idle, invalid input dan recovery | Tidak semua request-smuggling grammar |
| slow_request_head_is_cut_off | Deadline seluruh header | Ignored karena lama |
| file_descriptors_run_out | Recovery kehabisan FD | Behavior Linux/limit environment |
| routes_and_body_replace | Rewrite, replacement, streaming | Tambah semantic policy/output identity |
| block_exploits | Signature attack/benign lewat wire | Signature bukan jaminan WAF lengkap |
| server_header | Default/custom/off dan error | F10 tetap perlu selaraskan README |

Port helper mengambil rentang di bawah ephemeral range dan mengecek bind sebelum melepasnya; ini mengurangi benturan tetapi bukan reservation atomik sampai child bind. Tests memakai waktu tunggu/poll dan subprocess sehingga bisa flaky di mesin sibuk. Beberapa cabang curl hanya berjalan bila command tersedia; keberhasilan suite tanpa dependency opsional tidak selalu berarti jalur itu dieksekusi.

## fuzz/: target, seeds dan cara eksekusi

`fuzz/Cargo.toml` membuat workspace sendiri, menonaktifkan allocator default dan mengulang patch fork. Tujuh file `fuzz_targets/*.rs` masing-masing hanya macro libfuzzer ke wrapper library—seluruh semantic oracle berada di src/fuzzing.rs dan exploits helper.

`fuzz/run.sh` build nightly lalu menjalankan semua target paralel dengan default 300 detik, timeout input 10 detik dan RSS limit 2 GiB perprocess. Total penggunaan memori tujuh proses dapat jauh lebih besar dari 2 GiB. Seeds adalah input, bukan expected-output formal; nama generic sNNN perlu katalog attack/benign untuk investigasi regression yang lebih nyaman. Seluruh file seed tercatat perfile beserta hash di inventaris.

Tidak ada hasil fuzz baru yang diklaim di sini; mem-parsing seed bukan menjalankan libFuzzer.

## deploy/: sistem operasi dan ingestion log

`conduit.service` memakai user terpisah, capability bind port rendah, NoNewPrivileges, filesystem read-only dan batas kernel/device/namespace. Runtime/log/state directory diberikan melalui systemd. Readiness daemon diperlukan agar ExecReload tidak mematikan old process sebelum new siap mengambil listener.

Reload unit melakukan new run -d -u lalu QUIT MAINPID lama. Snapshot PIDFile dan urutan signal adalah area yang harus diuji dengan service manager nyata, bukan hanya cargo unit. Filter syscall pernah memengaruhi openat2 menurut komentar; uji efektif unit di distro sasaran. Security fallback F02 tetap relevan jika kernel/filter menolak syscall.

`conduit.logrotate` rename/create 0640, delaycompress dan grup adm mendukung writer reopening. Jika writer tidak menulis lama, pengecekan inode masih berjalan sehingga file akan dibuka berikutnya. Bukan copytruncate.

`crowdsec/conduit-logs.yaml` memetakan JSON schema log ke metadata parser; `acquis-conduit.yaml` mengambil access log dan `acquis-appsec.yaml` memakai sumber inspector. Perubahan field Entry harus dikoordinasikan dengan parser. Audit ini membaca config ingestion tetapi tidak menjalankan cscli/parser container.

## scripts/: pembuktian operasional dan performa

### scripts/bench.sh

Membangun setup nginx upstream, nginx NPM-style tanpa keepalive, nginx tuned dan conduit. CPU affinity memisahkan proxy/upstream/load generator. Mengukur HTTP, HTTPS, H2, handshake, serta fixed rate 50/70 persen. Gate memeriksa errors conduit, throughput relatif nginx dan p99 baseline/overhead.

Perbandingan rasio mengurangi noise host tetapi tidak menghilangkannya. Batas p99 at saturation sensitif queueing dan load generator. Baseline tidak di-version pada root (target ignored), sehingga perlu ekspor hasil + machine metadata untuk reviewer. Script menjalankan Docker/kill/cleanup; audit ini hanya syntax-check.

### scripts/ab.sh

Alternating variants beberapa putaran, median, parameter server config perbinary. Desain lebih baik untuk perubahan kecil daripada satu run sebelum/sesudah. CPU mask default berbeda bench.sh; catat mask aktual saat membandingkan angka dari dua tool. Config extras adalah input operator yang dipercaya, bukan interface publik.

### scripts/compare.py

Menulis konfigurasi tujuh jenis proxy, mengatur container cgroup CPU/memory, smoke, warmup, rotating order, fixed load terhadap proxy terlambat dan periksa line log. Ini memperluas konteks benchmark. Image tags seperti lts/latest tetap mutable; simpan resolved digest. Output errors/log mismatch belum semuanya fail-fast gate. Temporary cert diberi permission agar container dapat membaca; ini artefak benchmark sementara, bukan private key production.

### scripts/parity.py dan scripts/parity/stub.py

Parity mengambil hosts via regex terhadap bentuk YAML tertentu, membandingkan subset header/status dan opsional echo method/target/body. Regex ini bukan parser YAML umum: flow style/multiline/format lain dapat tidak terdeteksi. Normalisasi port, IP dan request ID sengaja menghapus perbedaan environment, tetapi bisa menyembunyikan bug authority jika dipakai sebagai satu-satunya oracle F03.

Stub memakai ThreadingHTTPServer dan membalas JSON representasi request; fixture localhost/test, bukan upstream production yang hardened. Body length memakai Content-Length, sehingga tidak semua chunked request diukur seperti origin nyata.

### scripts/pgo.sh dan scripts/pgo-train.sh

PGO script mencocokkan LLVM major profdata terhadap rustc, memakai target terpisah dan generate/merge/use profile. Training mencakup proxy, TLS/H2, cache, file, compression, plugins/errors/redirects. Profile yang representatif mengurangi optimisasi hanya untuk satu endpoint; tetap perlu validasi workload production lain. Membaca RUSTFLAGS dengan shell array split tidak mengimplementasikan parser quoting semua bentuk flag kompleks.

Training cleanup mempunyai kill -9 fallback; profile instrumented normalnya perlu shutdown baik agar profraw ditulis. Script memeriksa ada profile, tetapi existence saja tidak mengukur coverage atau profile stale mismatch. Test PGO binary harus sama ketatnya dengan release biasa.

### scripts/build-release.sh

Buildx mengekspor binary, menjalankan --version, menampilkan highest required glibc symbol dan shared dependencies, menolak dynamic ssl/crypto/libz lalu membuat sha256. Angka glibc ditampilkan, bukan dibandingkan programatis dengan 2.35. Container baseline memberi kontrol lebih utama, tetapi gate eksplisit akan lebih mudah direview.

### scripts/soak/deploy.sh, soak.sh, ws.py dan report.py

Deploy menjalankan transient unit pada host disposable, mengarsip run lama, mengirim binary/oha/harness dan dapat memasang nginx. Soak memberi mixed traffic serta jadwal reload, backend rejection, cert/log rotation dan upgrade. Sampel RSS/FD/thread/CPU/metrics dikumpulkan berkala.

ws.py adalah client/server fixture standard library untuk long/short-lived WebSocket, masking dan echo. Bukan implementasi RFC WebSocket lengkap untuk production; framing besar, fragmentation dan kontrol protokol belum semua dimodelkan.

report.py mengelompokkan failure berdasarkan jendela disruption dan menghitung RSS slope perprocess setelah warmup. Korelasi waktu tidak membuktikan kausalitas. CPU hitungan mengasumsikan tick 100 Hz pada komentar; untuk portable report catat CLK_TCK saat collection. Counter dropped/reuse_lost terakhir reset pada upgrade proses, sehingga angka terakhir bukan otomatis total sepanjang 24 jam.

Soak loop memakai set -uo tanpa -e agar gangguan tertentu tidak mengakhiri run. Tidak semua kegagalan child membuat harness gagal. Trap teardown akan meningkatkan cleanup jika ada abort. Tidak ada remote command soak yang dijalankan pada audit ini.

## docs/, vendor/, assets/ dan graphify-out/

`docs/configuration.md` adalah kontrak semantik yang lebih rinci daripada README: default, reload/restart, urutan request dan perplugin. `docs/technical.md` memuat keputusan desain, profiling, benchmark/test history dan roadmap. Angka di sana adalah hasil yang dilaporkan proyek, bukan hasil audit baru.

`vendor/README.md` merinci perubahan pooling, peer address syscall, MIME/header fast path, task pipe, module lookup, deadline slowloris, Server error header, readiness signal dan backport Brotli. Tiap perubahan perlu dibaca dengan diff terhadap upstream dan regression test fork. Folder vendor/pingora pada checkout kosong; graph juga sengaja mengecualikannya. Ini bagian terbesar yang belum dapat diaudit, karena parser/socket/cache engine sebenarnya berada di sana.

`assets/brand/README.md` menjelaskan logo Lanes/Ember, ukuran/warna/typography dan SVG/PNG/favicon. SVG merupakan aset statis; validasi XML memberi kepastian syntax, bukan penilaian visual/aksesibilitas menyeluruh. Binary PNG/ICO dicatat ukuran/hash, tidak diinterpretasikan sebagai source code. Tidak ditemukan aplikasi UI web yang memakai aset selain dokumentasi.

`graphify-out/graph.json`, `graph.html`, `GRAPH_REPORT.md` adalah artefak turunan. Dipakai untuk navigasi, bukan source of truth behavior. Cache/reflect metadata lokal tidak perlu disamakan dengan source perubahan aplikasi. Graph count/degree/confidence sudah dirangkum di laporan utama; tidak ada alasan menafsirkan setiap edge extracted sebagai jalur executable yang sudah terbukti dengan trace runtime.

## Titik pengembangan yang paling bernilai

Arsitektur dapat dipertahankan. Refactor yang beralasan adalah tipe path/authority terpisah, compiled representation fingerprint untuk cache, validasi konfigurasi terpusat yang lengkap, dan contract tests lintasmodul. Memecah file besar semata demi mengecilkan line count kurang bernilai dibanding memperjelas invariant dan ownership state.

Audit berikutnya sebaiknya dimulai dari F01/F02/F03, lalu menjalankan matriks cache/TLS/reload, dan terakhir membaca fork pada commit terkunci. Tanpa itu, jumlah test dan kualitas benchmark belum cukup untuk menyatakan seluruh trust boundary terverifikasi.
