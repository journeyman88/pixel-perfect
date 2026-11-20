// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Animation evaluation: flipbook playback, timeline state, the change ticks of
//! an animated resource and its sequence of merged frames.
//!
//! Every animated resource is evaluated on its own clock `F` (see
//! [`Animation::clock`]); a component with `fps` advances one frame-unit every
//! `F / fps` ticks.

use std::collections::HashMap;

use crate::compose::{self, Placed, find_panel, find_scene, is_animated_panel};
use crate::model::{ElementSource, Flipbook, Panel, Scene, Timeline, Transform, Transition};
use crate::raster::{self, Image};
use crate::render;
use crate::semantic::ResolvedWorksheet;
use crate::time::{self, Exceeded, Periodic};

/// Most frames an APNG can hold (`num_frames` is a 31-bit value).
pub const MAX_FRAMES: usize = (1 << 31) - 1;

/// True if the resource is rendered as an animation.
pub fn is_animated(ws: &ResolvedWorksheet, collection: &str, id: &str) -> bool {
    match collection {
        "flipbooks" | "timelines" => true,
        "panels" => find_panel(ws, id).is_some_and(is_animated_panel),
        "scenes" => find_scene(ws, id).is_some_and(|s| compose::is_animated_scene(ws, s)),
        _ => false,
    }
}

fn find_flipbook<'a>(ws: &'a ResolvedWorksheet, id: &str) -> &'a Flipbook {
    ws.model
        .flipbooks
        .iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("unknown flipbook {id:?}"))
}

fn find_timeline<'a>(ws: &'a ResolvedWorksheet, id: &str) -> &'a Timeline {
    ws.model
        .timelines
        .iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("unknown timeline {id:?}"))
}

// Flipbooks

/// Loop length of a flipbook in frame-units.
pub fn flipbook_units(fb: &Flipbook) -> u64 {
    fb.frames.iter().map(|f| u64::from(f.duration)).sum()
}

/// Frame-unit at which each frame starts.
fn frame_starts(fb: &Flipbook) -> Vec<u64> {
    fb.frames
        .iter()
        .scan(0, |acc, f| {
            let start = *acc;
            *acc += u64::from(f.duration);
            Some(start)
        })
        .collect()
}

/// Index of the frame shown at frame-unit `unit` (taken modulo the loop).
pub fn flipbook_frame(fb: &Flipbook, unit: u64) -> usize {
    let unit = unit % flipbook_units(fb);
    let starts = frame_starts(fb);
    starts.partition_point(|&s| s <= unit) - 1
}

// Timelines

/// The state of one panel element at a frame-unit of a timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementState {
    pub coordinates: [i64; 2],
    pub z_index: i64,
    pub visible: bool,
    pub transforms: Vec<Transform>,
    /// The last `seekFlipbook` applied, as `(at, frameIndex)`. `None` means the
    /// flipbook plays from the start of the timeline loop.
    pub seek: Option<(u64, usize)>,
}

/// `n / d` rounded to the nearest integer, halves away from zero (`d > 0`).
pub fn round_away(n: i128, d: i128) -> i128 {
    let q = (2 * n.abs() + d) / (2 * d);
    if n < 0 { -q } else { q }
}

/// Coordinates of the element `id` at frame-unit `t`, starting from `initial`.
fn position(timeline: &Timeline, id: &str, initial: [i64; 2], t: u64) -> [i64; 2] {
    let mut moves: Vec<(u64, u64, [i64; 2])> = timeline
        .transitions
        .iter()
        .filter_map(|tr| match tr {
            Transition::MoveTo {
                element,
                coordinates,
                start,
                duration,
            } if element == id => Some((*start, *duration, *coordinates)),
            _ => None,
        })
        .collect();
    moves.sort_by_key(|m| m.0);
    let mut p = initial;
    for (start, duration, p1) in moves {
        if t < start {
            break;
        }
        if t >= start + duration {
            p = p1;
            continue;
        }
        let step = |axis: usize| {
            let delta = i128::from(p1[axis]) - i128::from(p[axis]);
            let offset = round_away(delta * i128::from(t - start), i128::from(duration));
            (i128::from(p[axis]) + offset) as i64
        };
        return [step(0), step(1)];
    }
    p
}

/// The state of every element of `panel` (same order as `panel.elements`) at
/// frame-unit `t` of `timeline`.
pub fn timeline_state(timeline: &Timeline, panel: &Panel, t: u64) -> Vec<ElementState> {
    let mut states: Vec<ElementState> = panel
        .elements
        .iter()
        .map(|e| ElementState {
            coordinates: e.coordinates,
            z_index: e.z_index,
            visible: e.visible,
            transforms: e.transforms.clone(),
            seek: None,
        })
        .collect();
    let index_of = |id: &str| {
        panel
            .elements
            .iter()
            .position(|e| e.id.as_deref() == Some(id))
    };
    for j in timeline.instant_order() {
        let transition = &timeline.transitions[j];
        if transition.at().is_some_and(|at| at > t) {
            break;
        }
        let Some(k) = index_of(transition.element()) else {
            continue;
        };
        let state = &mut states[k];
        match transition {
            Transition::SetVisible { value, .. } => state.visible = *value,
            Transition::MoveToZ { value, .. } => state.z_index = *value,
            Transition::AppendTransforms { transforms, .. } => {
                state.transforms.extend_from_slice(transforms)
            }
            Transition::ResetTransforms { .. } => {
                state.transforms = panel.elements[k].transforms.clone()
            }
            Transition::SeekFlipbook {
                at, frame_index, ..
            } => state.seek = Some((*at, *frame_index)),
            Transition::MoveTo { .. } => unreachable!("not an instant transition"),
        }
    }
    for (state, element) in states.iter_mut().zip(&panel.elements) {
        if let Some(id) = &element.id {
            state.coordinates = position(timeline, id, element.coordinates, t);
        }
    }
    states
}

// Resources

#[derive(Debug, Clone, Copy)]
enum Kind<'a> {
    Flipbook(&'a Flipbook),
    Panel(&'a Panel),
    Timeline(&'a Timeline, &'a Panel),
    Scene(&'a Scene),
}

/// Errors found while setting up the time model of a resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeError {
    /// The clock or the loop length does not fit in 64 bits.
    TooLong,
}

/// An animated resource with its clock and loop length.
#[derive(Debug, Clone)]
pub struct Animation<'a> {
    ws: &'a ResolvedWorksheet,
    kind: Kind<'a>,
    /// Ticks per second.
    pub clock: u64,
    /// Ticks in one loop.
    pub loop_len: u64,
    pub width: u32,
    pub height: u32,
    /// Distinct `fps` values involved, ascending.
    pub rates: Vec<u32>,
}

/// Flipbooks drawn by the visible flipbook elements of a panel.
fn visible_flipbooks<'a>(ws: &'a ResolvedWorksheet, panel: &'a Panel) -> Vec<&'a Flipbook> {
    panel
        .elements
        .iter()
        .filter(|e| e.visible)
        .filter_map(|e| match e.source() {
            ElementSource::Flipbook(f) => Some(find_flipbook(ws, f)),
            ElementSource::Sprite(_) => None,
        })
        .collect()
}

/// Flipbooks of every flipbook element of a panel, visible or not.
fn all_flipbooks<'a>(ws: &'a ResolvedWorksheet, panel: &'a Panel) -> Vec<&'a Flipbook> {
    panel
        .elements
        .iter()
        .filter_map(|e| match e.source() {
            ElementSource::Flipbook(f) => Some(find_flipbook(ws, f)),
            ElementSource::Sprite(_) => None,
        })
        .collect()
}

impl<'a> Kind<'a> {
    fn rates(self, ws: &'a ResolvedWorksheet, out: &mut Vec<u32>) {
        match self {
            Kind::Flipbook(fb) => out.push(fb.fps),
            Kind::Panel(p) => out.extend(visible_flipbooks(ws, p).iter().map(|f| f.fps)),
            Kind::Timeline(t, p) => {
                out.push(t.fps);
                out.extend(all_flipbooks(ws, p).iter().map(|f| f.fps));
            }
            Kind::Scene(s) => {
                for layer in layer_kinds(ws, s).into_iter().flatten() {
                    layer.rates(ws, out);
                }
            }
        }
    }

    /// Loop length in ticks of `clock`.
    fn loop_len(self, ws: &'a ResolvedWorksheet, clock: u64) -> Option<u64> {
        let ticks = |units: u64, fps: u32| units.checked_mul(clock / u64::from(fps));
        match self {
            Kind::Flipbook(fb) => ticks(flipbook_units(fb), fb.fps),
            Kind::Panel(p) => visible_flipbooks(ws, p).into_iter().try_fold(1, |acc, fb| {
                time::lcm(acc, ticks(flipbook_units(fb), fb.fps)?)
            }),
            Kind::Timeline(t, _) => ticks(t.duration, t.fps),
            Kind::Scene(s) => layer_kinds(ws, s)
                .into_iter()
                .flatten()
                .try_fold(1, |acc, k| time::lcm(acc, k.loop_len(ws, clock)?)),
        }
    }

    fn size(self) -> (u32, u32) {
        match self {
            Kind::Flipbook(fb) => (fb.width, fb.height),
            Kind::Panel(p) | Kind::Timeline(_, p) => (p.width, p.height),
            Kind::Scene(s) => (s.width, s.height),
        }
    }
}

/// The animated kind of each scene layer, `None` for static panel layers.
fn layer_kinds<'a>(ws: &'a ResolvedWorksheet, scene: &'a Scene) -> Vec<Option<Kind<'a>>> {
    scene
        .layers
        .iter()
        .map(|layer| match (&layer.panel, &layer.timeline) {
            (Some(p), _) => {
                let panel = find_panel(ws, p).expect("validated reference");
                is_animated_panel(panel).then_some(Kind::Panel(panel))
            }
            (None, Some(t)) => {
                let timeline = find_timeline(ws, t);
                let panel = find_panel(ws, &timeline.panel).expect("validated reference");
                Some(Kind::Timeline(timeline, panel))
            }
            (None, None) => unreachable!("schema requires _panel or _timeline"),
        })
        .collect()
}

/// Ticks of one period of a flipbook played from tick 0.
fn flipbook_periodic(fb: &Flipbook, clock: u64) -> Periodic {
    let v = clock / u64::from(fb.fps);
    Periodic::new(
        flipbook_units(fb) * v,
        frame_starts(fb).into_iter().map(|s| s * v).collect(),
    )
}

/// Frame boundaries of a flipbook between ticks `start` and `end`, when it
/// shows frame-unit `offset` at `start` and advances one unit every `v` ticks.
fn flipbook_boundaries(
    fb: &Flipbook,
    v: u64,
    (start, end, offset): (u64, u64, u64),
    out: &mut Vec<u64>,
    limit: usize,
) -> Result<(), Exceeded> {
    let total = flipbook_units(fb);
    let starts = frame_starts(fb);
    let offset = offset % total;
    // Candidates grow with `cycle` and `b`, so the first one at or after `end`
    // ends the walk.
    for cycle in 0.. {
        let base = cycle * total;
        for &b in &starts {
            let Some(n) = (base + b).checked_sub(offset).filter(|&n| n > 0) else {
                continue;
            };
            let tick = start + n * v;
            if tick >= end {
                return Ok(());
            }
            out.push(tick);
            if out.len() > limit {
                return Err(Exceeded);
            }
        }
    }
    Ok(())
}

/// Ticks of one loop of a timeline at which its state or a flipbook element can
/// change.
fn timeline_periodic(
    ws: &ResolvedWorksheet,
    timeline: &Timeline,
    panel: &Panel,
    clock: u64,
    limit: usize,
) -> Result<Periodic, Exceeded> {
    let u = clock / u64::from(timeline.fps);
    let period = timeline.duration * u;
    let mut events = Vec::new();
    for transition in &timeline.transitions {
        match *transition {
            Transition::MoveTo {
                start, duration, ..
            } => {
                for t in start + 1..=(start + duration).min(timeline.duration - 1) {
                    events.push(t * u);
                    if events.len() > limit {
                        return Err(Exceeded);
                    }
                }
            }
            _ => events.extend(transition.at().map(|at| at * u)),
        }
    }
    let order = timeline.instant_order();
    for element in &panel.elements {
        let ElementSource::Flipbook(f) = element.source() else {
            continue;
        };
        let fb = find_flipbook(ws, f);
        let starts = frame_starts(fb);
        // Phase segments: (start tick, frame-unit shown at that tick).
        let mut segments: Vec<(u64, u64)> = vec![(0, 0)];
        for &j in &order {
            if let Transition::SeekFlipbook {
                element: target,
                frame_index,
                at,
            } = &timeline.transitions[j]
                && element.id.as_deref() == Some(target.as_str())
            {
                let seg = (at * u, starts[*frame_index]);
                if segments.last().is_some_and(|s| s.0 == seg.0) {
                    segments.pop();
                }
                segments.push(seg);
            }
        }
        let v = clock / u64::from(fb.fps);
        for (i, &(start, offset)) in segments.iter().enumerate() {
            let end = segments.get(i + 1).map_or(period, |s| s.0);
            events.push(start);
            flipbook_boundaries(fb, v, (start, end, offset), &mut events, limit)?;
        }
    }
    Ok(Periodic::new(period, events))
}

impl<'a> Kind<'a> {
    fn periodics(
        self,
        ws: &'a ResolvedWorksheet,
        clock: u64,
        limit: usize,
        out: &mut Vec<Periodic>,
    ) -> Result<(), Exceeded> {
        match self {
            Kind::Flipbook(fb) => out.push(flipbook_periodic(fb, clock)),
            Kind::Panel(p) => out.extend(
                visible_flipbooks(ws, p)
                    .into_iter()
                    .map(|fb| flipbook_periodic(fb, clock)),
            ),
            Kind::Timeline(t, p) => out.push(timeline_periodic(ws, t, p, clock, limit)?),
            Kind::Scene(s) => {
                for kind in layer_kinds(ws, s).into_iter().flatten() {
                    kind.periodics(ws, clock, limit, out)?;
                }
            }
        }
        Ok(())
    }
}

/// Caches of images that do not change over time.
#[derive(Default)]
struct Cache {
    sprites: HashMap<String, Image>,
    frames: HashMap<(String, usize), Image>,
    panels: HashMap<String, Image>,
}

impl Cache {
    fn sprite(&mut self, ws: &ResolvedWorksheet, id: &str) -> &Image {
        self.sprites
            .entry(id.to_owned())
            .or_insert_with(|| render::sprite(ws, id))
    }

    /// Frame `index` of `fb` on the flipbook canvas.
    fn frame(&mut self, ws: &ResolvedWorksheet, fb: &Flipbook, index: usize) -> Image {
        let key = (fb.id.clone(), index);
        if let Some(image) = self.frames.get(&key) {
            return image.clone();
        }
        let frame = &fb.frames[index];
        let image = raster::apply_all(self.sprite(ws, &frame.sprite), &frame.transforms);
        let canvas = compose::flatten(
            fb.width,
            fb.height,
            &[Placed {
                image: &image,
                coordinates: frame.coordinates,
                z_index: 0,
            }],
        );
        self.frames.insert(key, canvas.clone());
        canvas
    }

    fn static_panel(&mut self, ws: &ResolvedWorksheet, id: &str) -> Image {
        self.panels
            .entry(id.to_owned())
            .or_insert_with(|| compose::panel(ws, id))
            .clone()
    }
}

/// One element ready to be transformed and placed.
struct Drawn {
    image: Image,
    coordinates: [i64; 2],
    z_index: i64,
}

fn flatten_drawn(width: u32, height: u32, drawn: &[Drawn]) -> Image {
    let items: Vec<Placed> = drawn
        .iter()
        .map(|d| Placed {
            image: &d.image,
            coordinates: d.coordinates,
            z_index: d.z_index,
        })
        .collect();
    compose::flatten(width, height, &items)
}

impl<'a> Kind<'a> {
    /// The image at `tick` of the resource clock.
    fn render(self, ws: &'a ResolvedWorksheet, clock: u64, tick: u64, cache: &mut Cache) -> Image {
        match self {
            Kind::Flipbook(fb) => {
                let unit = tick / (clock / u64::from(fb.fps));
                cache.frame(ws, fb, flipbook_frame(fb, unit))
            }
            Kind::Panel(panel) => {
                let mut drawn = Vec::new();
                for element in panel.elements.iter().filter(|e| e.visible) {
                    let source = match element.source() {
                        ElementSource::Sprite(s) => cache.sprite(ws, s).clone(),
                        ElementSource::Flipbook(f) => {
                            let fb = find_flipbook(ws, f);
                            let unit = tick / (clock / u64::from(fb.fps));
                            cache.frame(ws, fb, flipbook_frame(fb, unit))
                        }
                    };
                    drawn.push(Drawn {
                        image: raster::apply_all(&source, &element.transforms),
                        coordinates: element.coordinates,
                        z_index: element.z_index,
                    });
                }
                flatten_drawn(panel.width, panel.height, &drawn)
            }
            Kind::Timeline(timeline, panel) => {
                let u = clock / u64::from(timeline.fps);
                let local = tick % (timeline.duration * u);
                let states = timeline_state(timeline, panel, local / u);
                let mut drawn = Vec::new();
                for (element, state) in panel.elements.iter().zip(&states) {
                    if !state.visible {
                        continue;
                    }
                    let source = match element.source() {
                        ElementSource::Sprite(s) => cache.sprite(ws, s).clone(),
                        ElementSource::Flipbook(f) => {
                            let fb = find_flipbook(ws, f);
                            let v = clock / u64::from(fb.fps);
                            let unit = match state.seek {
                                Some((at, index)) => frame_starts(fb)[index] + (local - at * u) / v,
                                None => local / v,
                            };
                            cache.frame(ws, fb, flipbook_frame(fb, unit))
                        }
                    };
                    drawn.push(Drawn {
                        image: raster::apply_all(&source, &state.transforms),
                        coordinates: state.coordinates,
                        z_index: state.z_index,
                    });
                }
                flatten_drawn(panel.width, panel.height, &drawn)
            }
            Kind::Scene(scene) => {
                let kinds = layer_kinds(ws, scene);
                let drawn: Vec<Drawn> = scene
                    .layers
                    .iter()
                    .zip(kinds)
                    .map(|(layer, kind)| Drawn {
                        image: match (kind, &layer.panel) {
                            (Some(kind), _) => kind.render(ws, clock, tick, cache),
                            (None, Some(p)) => cache.static_panel(ws, p),
                            (None, None) => unreachable!("timeline layers are animated"),
                        },
                        coordinates: layer.coordinates,
                        z_index: layer.z_index,
                    })
                    .collect();
                flatten_drawn(scene.width, scene.height, &drawn)
            }
        }
    }
}

impl<'a> Animation<'a> {
    /// The animation of resource `id` in `collection`. Panics if the resource
    /// is not animated (see [`is_animated`]).
    pub fn new(ws: &'a ResolvedWorksheet, collection: &str, id: &str) -> Result<Self, TimeError> {
        let kind = match collection {
            "flipbooks" => Kind::Flipbook(find_flipbook(ws, id)),
            "panels" => Kind::Panel(find_panel(ws, id).expect("validated reference")),
            "timelines" => {
                let timeline = find_timeline(ws, id);
                Kind::Timeline(
                    timeline,
                    find_panel(ws, &timeline.panel).expect("validated reference"),
                )
            }
            "scenes" => Kind::Scene(find_scene(ws, id).expect("validated reference")),
            other => unreachable!("{other} are not animated"),
        };
        let mut rates = Vec::new();
        kind.rates(ws, &mut rates);
        rates.sort_unstable();
        rates.dedup();
        let clock = time::lcm_all(rates.iter().map(|&r| u64::from(r))).ok_or(TimeError::TooLong)?;
        let loop_len = kind.loop_len(ws, clock).ok_or(TimeError::TooLong)?;
        let (width, height) = kind.size();
        Ok(Animation {
            ws,
            kind,
            clock,
            loop_len,
            width,
            height,
            rates,
        })
    }

    /// Sorted ticks in `[0, loop_len)` at which a new frame may start. Fails if
    /// there are more than `limit`.
    pub fn change_ticks(&self, limit: usize) -> Result<Vec<u64>, Exceeded> {
        let mut periodics = Vec::new();
        self.kind
            .periodics(self.ws, self.clock, limit, &mut periodics)?;
        time::change_ticks(&periodics, self.loop_len, limit)
    }

    /// The first pair of consecutive change ticks whose distance cannot be
    /// written as an APNG delay, as `(tick, ticks until the next change)`.
    pub fn delay_overflow(&self, ticks: &[u64]) -> Option<(u64, u64)> {
        segments(ticks, self.loop_len).find(|&(_, len)| !time::delay_fits(len, self.clock))
    }

    /// The image at `tick` of the loop, before `--scale`.
    pub fn render_at(&self, tick: u64) -> Image {
        self.kind
            .render(self.ws, self.clock, tick, &mut Cache::default())
    }

    /// The frames of one loop with their durations in ticks, merging
    /// consecutive identical frames when the merged delay still fits APNG.
    pub fn frames(&self) -> Vec<(Image, u64)> {
        let ticks = self.all_change_ticks();
        let lengths = segments(&ticks, self.loop_len).map(|(_, len)| len);
        self.merged(&ticks, lengths, |a, b| {
            time::delay_fits(a + b, self.clock).then_some(a + b)
        })
    }

    /// The frames of one loop with their WebP durations in milliseconds (see
    /// [`time::ms_durations`]): segments of 0 ms are skipped without being
    /// rendered, then consecutive identical frames are merged. Durations may
    /// exceed what one WebP frame can hold; the encoder splits them.
    pub fn ms_frames(&self) -> Vec<(Image, u128)> {
        let ticks = self.all_change_ticks();
        let durations = self.ms_durations(&ticks);
        self.merged(&ticks, durations.into_iter(), |a, b| Some(a + b))
    }

    /// Upper bound of the WebP frames written for `ticks` (from
    /// [`Self::change_ticks`]), counting every non-zero segment split into
    /// parts of at most `max_ms`. Merging only lowers the count.
    pub fn ms_frame_bound(&self, ticks: &[u64], max_ms: u128) -> u128 {
        self.ms_durations(ticks)
            .into_iter()
            .map(|d| d.div_ceil(max_ms))
            .sum()
    }

    fn all_change_ticks(&self) -> Vec<u64> {
        self.change_ticks(MAX_FRAMES)
            .expect("the export plan bounds the frame count")
    }

    fn ms_durations(&self, ticks: &[u64]) -> Vec<u128> {
        let ends: Vec<u64> = segments(ticks, self.loop_len)
            .map(|(start, len)| start + len)
            .collect();
        time::ms_durations(&ends, self.clock)
    }

    /// Renders the segment starting at each of `ticks`, with the matching
    /// duration from `durations`. Segments whose duration is zero are skipped
    /// without rendering. A frame identical to the previous one is merged
    /// into it when `merge` returns the merged duration.
    fn merged<D>(
        &self,
        ticks: &[u64],
        durations: impl Iterator<Item = D>,
        merge: impl Fn(D, D) -> Option<D>,
    ) -> Vec<(Image, D)>
    where
        D: Copy + Default + PartialEq,
    {
        let mut cache = Cache::default();
        let mut frames: Vec<(Image, D)> = Vec::new();
        for (&tick, duration) in ticks.iter().zip(durations) {
            if duration == D::default() {
                continue;
            }
            let image = self.kind.render(self.ws, self.clock, tick, &mut cache);
            if let Some(last) = frames.last_mut()
                && last.0 == image
                && let Some(sum) = merge(last.1, duration)
            {
                last.1 = sum;
                continue;
            }
            frames.push((image, duration));
        }
        frames
    }
}

/// `(start, length)` of each segment between consecutive change ticks.
fn segments(ticks: &[u64], loop_len: u64) -> impl Iterator<Item = (u64, u64)> + '_ {
    ticks.iter().enumerate().map(move |(i, &t)| {
        let next = ticks.get(i + 1).copied().unwrap_or(loop_len);
        (t, next - t)
    })
}
