use std::cmp::Ordering;

use super::data::{self, Stats, WEEKS};
use crate::panels::queue::time;

pub(super) const SHOWN: usize = WEEKS * 7;

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const STEPS: [f64; 9] = [1., 2., 5., 10., 20., 50., 100., 200., 500.];
const DAY: i64 = 86_400;
const IDLE: [&str; 3] =
    ["No agent worked in the last 24 h", "No agent worked in the last 7 days", "No agent has worked yet"];
const BRIEF: [&str; 3] =
    ["Under a minute busy in the last 24 h", "Under a minute busy in the last 7 days", "Under a minute busy so far"];

impl Stats {
    pub(super) fn seconds_ago(&self, ago: usize) -> f64 {
        self.blocks.get(ago).map_or(0., |b| b.agent_seconds.max(0.))
    }

    pub(super) fn oldest_first(&self) -> Vec<f64> {
        (0..SHOWN).rev().map(|ago| self.seconds_ago(ago)).collect()
    }

    pub(super) fn streak(&self) -> usize {
        (0..SHOWN).take_while(|&ago| self.seconds_ago(ago) > 0.).count()
    }

    pub(super) fn active(&self) -> usize {
        (0..SHOWN).filter(|&ago| self.seconds_ago(ago) > 0.).count()
    }

    pub(super) fn best(&self) -> Option<(usize, f64)> {
        (0..SHOWN).map(|ago| (ago, self.seconds_ago(ago))).filter(|(_, seconds)| *seconds > 0.).fold(
            None,
            |best, (ago, seconds)| match best {
                Some((_, most)) if most >= seconds => best,
                _ => Some((ago, seconds)),
            },
        )
    }
}

pub(super) fn month_of(ago: usize, now: i64) -> u32 {
    time::local(ended(ago, now)).map_or(0, |civil| civil.month)
}

pub(super) fn day_name(ago: usize, now: i64) -> String {
    if ago == 0 {
        return "last 24 h".into();
    }
    time::local(ended(ago, now)).map(|civil| format!("{} {}", month_name(civil.month), civil.day)).unwrap_or_default()
}

pub(super) fn day_tip(ago: usize, seconds: f64, now: i64) -> String {
    let worked = if seconds > 0. { data::duration(seconds) } else { "no agent time".into() };
    format!("{} · {worked}", day_name(ago, now))
}

fn ended(ago: usize, now: i64) -> i64 {
    now - i64::try_from(ago).unwrap_or(0) * DAY
}

pub(super) fn month_name(month: u32) -> &'static str {
    month.checked_sub(1).and_then(|i| MONTHS.get(i as usize)).copied().unwrap_or("")
}

pub(super) fn month_marks(months: &[u32], room: usize) -> Vec<(usize, &'static str)> {
    let starts: Vec<usize> = (1..months.len()).filter(|&i| months[i] != months[i - 1]).collect();
    let first_fits = !months.is_empty() && starts.first().is_none_or(|&next| next >= room);
    first_fits.then_some(0).into_iter().chain(starts).map(|i| (i, month_name(months[i]))).collect()
}

#[expect(clippy::cast_precision_loss, reason = "a counter far below 2^52")]
pub(super) fn real(n: u64) -> f64 {
    n as f64
}

#[expect(clippy::cast_precision_loss, reason = "a position drawn on screen from a few hundred steps")]
pub(super) fn along(i: usize, last: usize) -> f32 {
    if last == 0 { 0. } else { (i.min(last) as f32) / (last as f32) }
}

pub(super) fn trailing_mean(values: &[f64], span: usize) -> Vec<f64> {
    let span = span.max(1);
    (0..values.len())
        .map(|i| {
            let from = (i + 1).saturating_sub(span);
            let shown = &values[from..=i];
            shown.iter().sum::<f64>() / f64::from(u32::try_from(shown.len()).unwrap_or(u32::MAX))
        })
        .collect()
}

pub(super) fn scale(most: f64) -> (f64, f64) {
    let step = STEPS.iter().copied().find(|step| step * 4. >= most).unwrap_or(STEPS[STEPS.len() - 1]);
    let top = (most / step).ceil().max(1.) * step;
    (top, step)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a whole factor clamped to a small positive range"
)]
pub(super) fn gauge_top(factor: f64) -> u32 {
    let whole = factor.clamp(0., 1_000.).ceil() as u32;
    whole.next_multiple_of(2).max(4)
}

pub(super) fn gauge_ticks(top: u32) -> Vec<u32> {
    let step = top.div_ceil(6).max(1);
    (0..=top).step_by(step as usize).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Versus {
    pub words: String,
    pub way: Ordering,
}

pub(super) fn versus(day: f64, week: f64) -> Option<Versus> {
    let average = week / 7.;
    if average <= 0. {
        return None;
    }
    let percent = ((day / average - 1.) * 100.).round();
    let way = if percent.abs() < 1. { Ordering::Equal } else { percent.total_cmp(&0.) };
    let words = match way {
        Ordering::Equal => "on your 7-day average".to_string(),
        Ordering::Greater => format!("{percent:.0}% above your 7-day average"),
        Ordering::Less => format!("{:.0}% below your 7-day average", percent.abs()),
    };
    Some(Versus { words, way })
}

pub(super) fn per_day(total: f64, days: u64) -> f64 {
    total / f64::from(u32::try_from(days.max(1)).unwrap_or(u32::MAX))
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a positive average shown whole")]
pub(super) fn decimal(n: f64) -> String {
    if n < 10. { format!("{n:.1}") } else { data::count(n.round() as u64) }
}

pub(super) fn idle_words(window: usize, agent_seconds: f64) -> &'static str {
    let words = if agent_seconds > 0. { BRIEF } else { IDLE };
    words[window.min(words.len() - 1)]
}

pub(super) fn busy_words(summed: bool) -> (&'static str, &'static str) {
    if summed { ("busy, summed over machines", "busy, summed") } else { ("on the clock", "wall time busy") }
}

#[cfg(test)]
mod tests;
