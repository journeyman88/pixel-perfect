## Context

The animation engine (`animate.rs`) models every animated resource as a
`Kind`: `Flipbook`, `Panel`, `Timeline(timeline, panel)` and `Scene`. Each
kind provides five things:

- its rates (`rates`), which the clock is the LCM of;
- its loop length (`loop_len`);
- its change ticks (`periodics`);
- its size;
- an image at any tick (`render`).

`Animation` wraps a kind and drives the shared frame logic: change ticks,
merging identical frames, APNG delays and WebP milliseconds.

A timeline animates the *elements* of one panel. `timeline_state` computes
each element's state at a frame-unit: instant transitions in `instant_order`,
then `position` for `moveTo`. Scenes draw layers at fixed coordinates and
`zIndex`. Each layer is either a static panel (cached) or an animated kind
rendered at the global tick.

A sequence is to a scene what a timeline is to a panel, restricted to the
three actions that make sense for whole layers. The schema is already drafted
in `schema/worksheet.schema.json`. The spec deltas define the behavior:
global time, loop = LCM, notice for longer loops, and layer `visible`.

## Goals / Non-Goals

**Goals:**
- `Kind::Sequence(sequence, scene)`, integrated in clock, loop, change ticks,
  rendering, export and manifest like any other animated resource.
- One implementation of the movement and instant-ordering rules, shared by
  timelines and sequences, so the two cannot drift apart.
- Layer `visible` honored everywhere: static scenes, animated scenes and
  sequences.

**Non-Goals:**
- Transforms on layers (`appendTransforms` and friends), seeking a layer's
  timeline, or easing curves.
- Sequences as layers of other scenes (nesting sequences).
- Restarting nested animations at each sequence loop. Time is global, as
  decided.

## Decisions

### 1. Model

```rust
pub struct Sequence { id, description, #[serde(rename="_scene")] scene, fps: u32, duration: u64, actions: Vec<Action> }

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    MoveTo   { #[serde(rename="_layer")] layer: String, coordinates: [i64; 2], start: u64, duration: u64 },
    SetVisible { #[serde(rename="_layer")] layer: String, value: bool, at: u64 },
    MoveToZ  { #[serde(rename="_layer")] layer: String, value: i64, at: u64 },
}
```

`Layer` gains `id: Option<String>` and `visible: bool` (`#[serde(default =
"default_true")]`, as for elements). `Worksheet` gains `sequences: Vec<Sequence>`.

*Alternative:* reuse `Transition` with a generic target field. Rejected: the
`_element`/`_layer` key names differ, and the narrower enum makes the
removed kinds unrepresentable.

### 2. Shared state rules

The two rules are extracted from `timeline_state`/`position` into small
generic helpers over a "move" view:

- `fn moved(moves: impl Iterator<Item = (start, duration, p1)>, initial, t) -> [i64; 2]`
  implements the linear movement with `round_away`;
- `fn instant_order<T>(items: &[T], at: impl Fn(&T) -> Option<u64>) -> Vec<usize>`
  orders instant actions by `at`, with ties in array order.

`timeline_state` and the new `sequence_state(sequence, scene, t) ->
Vec<LayerState>` both call them. `LayerState` holds `coordinates`, `z_index`
and `visible`. The existing timeline tests and golden files guard the
refactor: their output must not change.

### 3. Clock, loop and drawable layers

`Kind::Sequence` takes its rates from the sequence's `fps` and from the
rates of every **drawable** animated layer. A layer is drawable if it starts
visible, or if a `setVisible { value: true }` targets its id. The loop is
`lcm(duration · clock/fps, loops of drawable animated layers)`.

`layer_kinds` now ignores hidden layers for plain scenes (`visible: false`)
and takes a predicate for sequences (drawable). The current fixtures have no
hidden layer, so scene rendering is unchanged for them.

### 4. Change ticks

A new `sequence_periodic` mirrors `timeline_periodic`, without the flipbook
phase handling (layers have no seek). Its events are every frame-unit
boundary inside each `moveTo` and every `at`, all with period
`duration · u`. The drawable animated layers contribute their own periodics
through `Kind::periodics`, exactly as in `Kind::Scene`. `Periodic` already
expands everything over the LCM loop.

### 5. Rendering

`Kind::Sequence::render(tick)` computes `t = (tick mod (duration·u)) / u` and
`sequence_state(t)`. It then draws each visible layer at its state's
coordinates and `zIndex`:

- an animated layer is rendered at the **global** `tick`, not at `t`;
- a static layer comes from `Cache::static_panel`.

It composites with `flatten_drawn`. Static scenes (`compose::scene`) skip
hidden layers.

### 6. Validation

In `semantic.rs`:

- unique layer ids per scene, pointing to `/scenes/i/layers/j/id`;
- `_scene` resolves;
- `_layer` resolves among the scene's layer ids;
- the timing checks reuse the timeline ones, through a small shared function
  parameterized by the pointer prefix (`/sequences/i/actions/j`).

`exports` resolution learns the `sequences` collection.

### 7. Export, size guard, notice

- `COLLECTIONS` gains `sequences`; `is_animated` returns true for it.
- The size checker treats a sequence as its scene, with the same canvas.
- After a sequence's `Animation` is built, if
  `loop_len != duration · clock/fps`, the plan pushes the notice. Lengths are
  written with `time::reduced` as `n` or `n/d` seconds. `Plan.notices` exists
  but is unused so far; `main.rs` already prints it.
- The manifest needs no change: it is keyed by pointer.

### 8. Example and docs

- `examples/parallax.yaml`: three planes moving at different speeds over a
  static sky, plus a light layer toggled with `setVisible` and lifted with
  `moveToZ`. It is rendered by `tests/examples.rs`.
- README: sequences in Concepts and in the Animation section (global time,
  loop notice).

## Risks / Trade-offs

- [The LCM loop gets long, e.g. 7 s with 0.75 s gives 21 s, and the files get
  larger] → the notice says so explicitly. The existing size guard and the
  APNG/WebP frame limits still apply before anything is written.
- [Refactoring `timeline_state` changes timeline output by accident] → the
  timeline unit tests, the APNG/WebP golden files and the examples must stay
  byte-identical. This is checked as a task before sequences are added.
- [`visible: false` on layers now hides them: BREAKING] → no example or
  fixture uses it. The README mentions the change.
- [A seamless parallax needs content that wraps around] → out of scope for
  the engine. The example shows the pattern: a plane twice as wide, built
  with a `tile` transform, moved by exactly one tile width per loop.

## Migration Plan

Additive except the layer `visible` fix. No data migration; rollback means
removing the `sequences` collection from the schema and the engine.

## Open Questions

- Should a sequence be usable as a layer of another scene? This is left out:
  it would make loops recursive across scenes, and nobody needs it yet.
