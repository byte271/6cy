# BENCHMARK.md (Experimental)

**Build:** release  
**Version:** v0.1.1  
**Datasets:** Silesia Corpus, Canterbury Corpus

> These results are experimental and intended to validate format behavior (streaming, recovery, container overhead, CAS deduplication). They are not presented as a definitive performance comparison against other tools. Reproducible scripts and raw outputs are available in the repository.

---

## Test procedure (summary)

For each file in the dataset we ran:

```powershell
# example (Windows)
.\6cy.exe pack <input> --output <out>.6cy --codec zstd
.\6cy.exe unpack <out>.6cy -C <outdir>
```

We measured:
- Original size (bytes)
- Compressed size (bytes)
- Ratio = original / compressed (displayed as "x")
- Compress Time (seconds)
- Decompress Time (seconds)

All tests were executed with the current reference implementation (release build). No proprietary codecs were used — only the included standard codecs (e.g. zstd as the codec in these runs).

---

## Silesia Corpus results

| File     | Original (bytes) | Compressed (bytes) | Ratio | Compress Time (s) | Decompress Time (s) |
|----------|------------------:|--------------------:|------:|-------------------:|---------------------:|
| dickens  | 10,192,446        | 3,676,137           | 2.7726x | 0.0790844 | 0.0407387 |
| mozilla  | 51,220,480        | 18,387,449          | 2.7856x | 0.2615351 | 0.1249447 |
| mr       | 9,970,564         | 3,547,366           | 2.8107x | 0.0659680 | 0.0314705 |
| nci      | 33,553,445        | 2,852,540           | 11.7627x | 0.0769734 | 0.0628111 |
| ooffice  | 6,152,192         | 3,137,168           | 1.9611x | 0.0571361 | 0.0228715 |
| osdb     | 10,085,684        | 3,512,972           | 2.8710x | 0.0608535 | 0.0307915 |
| reymont  | 6,627,202         | 1,946,635           | 3.4044x | 0.0452743 | 0.0241714 |
| samba    | 21,606,400        | 4,988,262           | 4.3314x | 0.0869897 | 0.0516256 |
| sao      | 7,251,944         | 5,544,079           | 1.3081x | 0.0711665 | 0.0283026 |
| webster  | 41,458,703        | 12,171,007          | 3.4063x | 0.2456822 | 0.1398008 |
| x-ray    | 8,474,240         | 6,084,936           | 1.3927x | 0.1852616 | 0.0242496 |
| xml      | 5,345,280         | 636,202             | 8.4019x | 0.0256001 | 0.0202536 |

---

## Canterbury Corpus results

| File         | Original (bytes) | Compressed (bytes) | Ratio | Compress Time (s) | Decompress Time (s) |
|--------------|------------------:|--------------------:|------:|-------------------:|---------------------:|
| alice29.txt  | 152,089           | 56,094              | 2.7113x | 0.0107521 | 0.0087045 |
| asyoulik.txt | 125,179           | 50,298              | 2.4887x | 0.0197579 | 0.0096042 |
| cp.html      | 24,603            | 8,764               | 2.8073x | 0.0078371 | 0.0104992 |
| fields.c     | 11,150            | 3,705               | 3.0094x | 0.0145774 | 0.0107033 |
| grammar.lsp  | 3,721             | 1,605               | 2.3184x | 0.0000000 | 0.0110102 |
| kennedy.xls  | 1,029,744         | 112,134             | 9.1832x | 0.0185211 | 0.0155495 |
| lcet10.txt   | 426,754           | 141,329             | 3.0196x | 0.0115862 | 0.0206156 |
| plrabn12.txt | 481,861           | 191,991             | 2.5098x | 0.0107512 | 0.0131578 |
| ptt5         | 513,216           | 54,717              | 9.3795x | 0.0094389 | 0.0118542 |
| SHA1SUM      | 569               | 675                 | 0.8430x | 0.0090577 | 0.0130812 |
| sum          | 38,240            | 13,687              | 2.7939x | 0.0048199 | 0.0124780 |
| xargs.1      | 4,227             | 2,129               | 1.9854x | 0.0114831 | 0.0102572 |

---

## CAS Deduplication overhead

When packing archives with duplicate content, the CAS engine eliminates redundant block writes. The BLAKE3 hash lookup adds negligible overhead compared to compression time. Archives with high duplication rates will see significantly reduced output sizes with no decompression overhead on reads.

---

## Notes

- These results are from the current experimental reference implementation (release build, v0.1.1).
- The test focus was on validating streaming behavior, container overhead, and recovery semantics rather than producing definitive cross-tool comparisons.
- Values are presented as recorded (times in seconds). Users are encouraged to reproduce runs using the benchmark scripts in the repository for full raw output.

---

## Documentation Note

This document was compiled using automated tooling to organize raw benchmark
outputs into a readable format. All measurements and data are directly produced
by the test runs.
