## 1. Load-time validation of timelines

- [x] 1.1 Schema and model: `seekFlipbook` transition (`frameIndex` optional, default 0; `at`); internal `$defs/frameIndex` renamed to `frameUnit`
- [x] 1.2 Semantic check: `seekFlipbook` targets a `_flipbook` element and `frameIndex` < number of `frames`
- [x] 1.3 Tests: schema rejects negative `frameIndex`, missing `at`, extra fields; semantic "Seek on a sprite element" and "Frame index out of range"; `seekFlipbook` added to `tests/fixtures/valid/full.json`
- [x] 1.4 Semantic check: `at` and `start` < timeline `duration`; `moveTo` `start + duration` ≤ timeline `duration` (error on that `duration`)
- [x] 1.5 Semantic check: no overlapping `moveTo` on the same element (ordered by `start`; error on the later `start`)
- [x] 1.6 Tests: every "Timeline transitions are consistent" scenario of `specs/worksheet-loading/spec.md` not covered by 1.3
- [x] 1.7 Enumerate the chains of each timeline element (own `transforms` + `appendTransforms` since the last `resetTransforms`, by `at`, ties in array order) and run `check_crops` on each; errors point to the `area` inside the `appendTransforms` entry
- [x] 1.8 Tests: every "Transforms stay within bounds" scenario, including the three appended-chain ones; update the requirement wording in comments/messages that still say "static"

## 2. Time model

- [x] 2.1 `time.rs`: `gcd`/`lcm` on `u128` with checked overflow; clock `F` of a resource (transitive `fps` of flipbooks and timelines involved)
- [x] 2.2 Loop lengths in ticks: flipbook, timeline, animated panel (visible flipbook elements only), animated scene (animated layers only)
- [x] 2.3 Tests: every "Common clock" and "Loop lengths" scenario of `specs/animation/spec.md`; overflow returns an error instead of panicking
- [x] 2.4 Change ticks of a resource in `[0, loop)`: tick 0, flipbook frame boundaries (local time, including phase resets and seeks inside timelines), timeline frame-unit boundaries inside `moveTo` intervals, `at` of instant transitions, loop restarts of components; enumeration with an early stop at a given count
- [x] 2.5 Tests: change ticks for a flipbook, a timeline with `setVisible` only, a timeline with `moveTo`, a scene with two layers at coprime rates; early stop returns "exceeded"
- [x] 2.6 Delay fractions: `k / F` reduced; numerator split into `65535/d` chunks plus remainder; plan-time check that every segment between consecutive change ticks has a reduced denominator ≤ 65535 (error listing the resource's `fps` values); merge only when the merged denominator stays ≤ 65535
- [x] 2.7 Tests: every "Exact frame delays" scenario of `specs/apng-export/spec.md`; exhaustive test (no new dev-dependency) over every pair of `fps` in 1..=100 that boundaries of two directly played flipbooks give denominators ≤ 10000; the 97/89/83 scene with a seek fails at plan time

## 3. Animation evaluation

- [x] 3.1 `animate.rs`: flipbook frame lookup at local time (`u = (τ · fps / F) mod total`, cumulative intervals); flipbook frame rendering on its canvas with frame `transforms`, placement and clipping
- [x] 3.2 Tests: every "Flipbook playback" scenario
- [x] 3.3 Timeline state at frame-unit `t`: instant transitions by `at` then array order; `moveTo` with `round_away` per axis (`i64`); `p0` from the previous `moveTo` or the panel
- [x] 3.4 Tests: every "Timeline evaluation", "Instant transitions" and "Linear movement" scenario (positions checked on the state, not only on pixels)
- [x] 3.5 Flipbook phase in timelines: restart at every timeline loop, `seekFlipbook` sets the phase to the start of `frames[frameIndex]`, phase independent of visibility
- [x] 3.6 Tests: every "Flipbook phase in timelines" and "Flipbook elements" scenario
- [x] 3.7 `compose.rs`: `panel_at` / `scene_at` rendering a panel or scene from an explicit element/layer state; `panel` and `scene` become the time-0 static case (existing tests unchanged)
- [x] 3.8 Tests: "Frames use the static rules" and "Timeline layer flattened as a group" scenarios of `specs/composition/spec.md`; "Timeline over a static panel"
- [x] 3.9 Frame sequence: one frame per change-tick segment, merge consecutive identical frames (after scale), durations in ticks
- [x] 3.10 Tests: every "Frame sequence" and "Animated scenes" scenario; durations always sum to the loop

## 4. APNG encoding

- [x] 4.1 `encode.rs`: `choose` takes an iterator of images (frames in order, row-major each); PNG path unchanged
- [x] 4.2 Tests: every "APNG encoding chosen by color count" scenario; existing PNG palette tests still pass
- [x] 4.3 `encode::apng(frames, clock)`: `acTL` (`num_plays` 0), `IDAT` as frame 0, full-canvas `fcTL` with `dispose_op` NONE and `blend_op` SOURCE, `fdAT` for the rest, shared `PLTE`/`tRNS`, same compression settings as `encode::png`, no other chunks
- [x] 4.4 Tests: every "APNG structure" and "Lossless and deterministic APNG" scenario, decoding with the `png` crate (frame pixels, delays, chunk list); single-frame APNG; split long still

## 5. Export

- [x] 5.1 Export selection: all collections exportable; static → PNG, animated → APNG; remove the "not supported yet" error and notice
- [x] 5.2 Tests: every "Export selection" scenario of `specs/png-export/spec.md`; update the CLI tests that expected the notice or the error
- [x] 5.3 Plan: for animated exports compute the segment count `S` (early stop) and check `S × w × h × 4 × scale²` against `--max-image-size`; more than 2³¹ − 1 frames is an error even with `none`; size walk includes transforms added by `appendTransforms`
- [x] 5.4 Tests: "Appended transform", "Long animation" and "No limit" scenarios of "Image size limit" (planning only, no large allocation); nothing written on error
- [x] 5.5 Manifest: `frames` and `loop` (`{ "num", "den" }`, reduced) for animated entries; static entries unchanged
- [x] 5.6 Tests: every "Manifest" scenario, including "Animated entry"; byte-identical manifest on repeated render

## 6. Wrap-up

- [x] 6.1 Golden files: regenerate `full` and `full-x4-safe` (now including the flipbook, the timeline and the animated scene); add a golden fixture with a timeline using every transition kind and a scene with two timeline layers at different `fps`
- [x] 6.2 `examples/`: an animated example (a timeline with a walking flipbook, `moveTo` seamless loop and `seekFlipbook`) plus an entry in `examples/README.md`; covered by `tests/examples.rs`
- [x] 6.3 README: animation in Status and Render, the time model and the `moveTo` rounding rule in one paragraph each, APNG output and the new manifest fields
- [x] 6.4 `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass
