# STUDIO-BENCH-1 Baseline

The release acceptance benchmark uses five unrecorded warm-ups followed by 30 plugin launch
samples, then ten interaction warm-ups followed by a fixed 100-operation catalog/cart corpus.
Durations cover host event dispatch through the committed retained/native state available for the
next present. Route/property transitions are host-clock scheduled against a 16,667 µs frame
budget. Idle work compares runtime counters without injecting an event.

The formal `STUDIO-BENCH-1` device is an Intel Processor N100 with 8 GiB RAM, integrated Intel
UHD graphics, NVMe storage, a 1920x1080@60 output, and native Weston. Results from other devices
must be labeled informative and cannot approve a release.

Release limits:

- warm first usable frame median: at most 150,000 µs;
- ordinary interaction p95: at most 100,000 µs;
- single-property patch p95: at most 2,000 µs;
- 100-property patch batch p95: at most 8,000 µs;
- host transition sampling p95: at most one 16,667 µs frame budget;
- idle repeated guest/render work: exactly zero.

Run `./scripts/benchmark-acceptance.sh`. The validation report records dated values and baseline
hardware; results from other hardware are informative until that baseline is updated explicitly.
