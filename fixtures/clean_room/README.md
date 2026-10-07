# Synthetic Designer XML observations

These minimal inputs were authored for this repository from the QName/value
observations in `docs/legal/bsl-metadata-clean-room-slice.md` §3.3 (X1–X4,
X6–X8, X10–X11, X13–X14). No private configuration or upstream fixture was copied.
Names and UUIDs are invented; the files are MIT-licensed test inputs.

`crates/bsl-metadata/tests/clean_room_contracts.rs` varies the observed inputs to
check the preserved fallback/error/presence contracts. These inputs live outside
`crates/bsl-metadata/fixtures`, the fixed corpus sampled at commit `85c3de81`, so
they do not change the source set of the published observations. `Year` and `Quarter` are observed periodicity
values, but deliberately remain unsupported by the runtime model. `Own` is a
preserved accepted token, not a claim that it occurred in the surveyed corpus.
