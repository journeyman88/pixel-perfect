//! Exact time arithmetic for animations: clocks, loop lengths, change ticks and
//! APNG delays. Everything is integer; a tick is `1 / F` seconds for the clock
//! `F` of the resource being rendered.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Largest value of an APNG `delay_num` or `delay_den`.
pub const MAX_DELAY_PART: u64 = u16::MAX as u64;

pub fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Least common multiple, `None` on overflow of `u64`.
pub fn lcm(a: u64, b: u64) -> Option<u64> {
    if a == 0 || b == 0 {
        return Some(0);
    }
    let (a, b) = (u128::from(a), u128::from(b));
    u64::try_from(a / gcd(a, b) * b).ok()
}

/// Least common multiple of all values (1 for none), `None` on overflow.
pub fn lcm_all(values: impl IntoIterator<Item = u64>) -> Option<u64> {
    values.into_iter().try_fold(1, lcm)
}

/// A set of ticks repeating with `period`: `events` are the ticks of one period,
/// sorted, unique, all in `[0, period)`, and include 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Periodic {
    pub period: u64,
    pub events: Vec<u64>,
}

impl Periodic {
    pub fn new(period: u64, mut events: Vec<u64>) -> Periodic {
        events.push(0);
        events.retain(|&e| e < period);
        events.sort_unstable();
        events.dedup();
        Periodic { period, events }
    }
}

/// More change ticks than the caller allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exceeded;

/// The sorted, unique ticks in `[0, loop_len)` at which some sequence has an
/// event. Fails as soon as more than `limit` ticks are found, so a huge loop
/// costs at most `limit` steps.
pub fn change_ticks(
    sequences: &[Periodic],
    loop_len: u64,
    limit: usize,
) -> Result<Vec<u64>, Exceeded> {
    // (tick, sequence, repetition, index in the period)
    let mut heap: BinaryHeap<Reverse<(u64, usize, u64, usize)>> = sequences
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.events.is_empty())
        .map(|(i, s)| Reverse((s.events[0], i, 0, 0)))
        .collect();
    let mut ticks: Vec<u64> = Vec::new();
    if sequences.is_empty() {
        ticks.push(0);
    }
    while let Some(Reverse((tick, s, rep, idx))) = heap.pop() {
        if tick >= loop_len {
            continue;
        }
        if ticks.last() != Some(&tick) {
            if ticks.len() == limit {
                return Err(Exceeded);
            }
            ticks.push(tick);
        }
        let seq = &sequences[s];
        let (rep, idx) = if idx + 1 < seq.events.len() {
            (rep, idx + 1)
        } else {
            (rep + 1, 0)
        };
        if let Some(next) = rep
            .checked_mul(seq.period)
            .and_then(|base| base.checked_add(seq.events[idx]))
        {
            heap.push(Reverse((next, s, rep, idx)));
        }
    }
    Ok(ticks)
}

/// `ticks / clock` reduced to lowest terms, as `(numerator, denominator)`.
pub fn reduced(ticks: u64, clock: u64) -> (u64, u64) {
    let g = gcd(u128::from(ticks), u128::from(clock)) as u64;
    (ticks / g, clock / g)
}

/// True if a frame of `ticks` can be written with an APNG denominator.
pub fn delay_fits(ticks: u64, clock: u64) -> bool {
    reduced(ticks, clock).1 <= MAX_DELAY_PART
}

/// APNG delays (`delay_num`, `delay_den`) for a frame of `ticks` on a clock of
/// `clock` ticks per second: the reduced fraction, split into several delays
/// when the numerator does not fit in 16 bits. Returns `None` if the
/// denominator does not fit.
pub fn delays(ticks: u64, clock: u64) -> Option<Vec<(u16, u16)>> {
    let (n, d) = reduced(ticks, clock);
    let d16 = u16::try_from(d).ok()?;
    let mut out = vec![(u16::MAX, d16); (n / MAX_DELAY_PART) as usize];
    let rest = n % MAX_DELAY_PART;
    if rest != 0 {
        let (rn, rd) = reduced(rest, d);
        out.push((rn as u16, rd as u16));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcm_overflow_is_none() {
        assert_eq!(lcm(u64::MAX, u64::MAX - 1), None);
        assert_eq!(lcm_all([12, 8]), Some(24));
        assert_eq!(lcm_all([]), Some(1));
    }

    #[test]
    fn merge_of_periodic_sequences() {
        let a = Periodic::new(4, vec![2]);
        let b = Periodic::new(6, vec![3]);
        assert_eq!(
            change_ticks(&[a, b], 12, 100),
            Ok(vec![0, 2, 3, 4, 6, 8, 9, 10])
        );
    }

    #[test]
    fn limit_stops_early() {
        let a = Periodic::new(1, vec![]);
        assert_eq!(
            change_ticks(std::slice::from_ref(&a), u64::MAX, 10),
            Err(Exceeded)
        );
        assert_eq!(change_ticks(&[a], 10, 10).map(|t| t.len()), Ok(10));
    }

    #[test]
    fn delays_split_long_numerators() {
        assert_eq!(delays(6, 24), Some(vec![(1, 4)]));
        assert_eq!(delays(70000, 1), Some(vec![(65535, 1), (4465, 1)]));
        assert_eq!(delays(65535 * 2, 1), Some(vec![(65535, 1), (65535, 1)]));
        assert_eq!(delays(1, 716_539), None);
    }
}
