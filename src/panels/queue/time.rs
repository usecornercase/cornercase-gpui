use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Civil {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub weekday: u32,
}

impl Civil {
    pub(super) const EPOCH: Civil = Civil { year: 1970, month: 1, day: 1, hour: 0, minute: 0, weekday: 4 };

    pub(crate) fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub(crate) fn clock(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

pub(crate) fn local(at: i64) -> Option<Civil> {
    let t = libc::time_t::try_from(at).ok()?;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&raw const t, &raw mut tm).is_null() };
    ok.then(|| Civil {
        year: tm.tm_year + 1900,
        month: u32::try_from(tm.tm_mon + 1).unwrap_or(1),
        day: u32::try_from(tm.tm_mday).unwrap_or(1),
        hour: u32::try_from(tm.tm_hour).unwrap_or(0),
        minute: u32::try_from(tm.tm_min).unwrap_or(0),
        weekday: u32::try_from(tm.tm_wday).unwrap_or(0),
    })
}

pub(crate) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

pub(crate) fn days(year: i32, month: u32, day: u32) -> i64 {
    let y = i64::from(year) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub(crate) fn when_label(at: Civil, today: Civil) -> String {
    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let clock = at.clock();
    match days(at.year, at.month, at.day) - days(today.year, today.month, today.day) {
        0 => format!("today {clock}"),
        1 => format!("tomorrow {clock}"),
        -1 => format!("yesterday {clock}"),
        2..=6 => format!("{} {clock}", WEEKDAYS.get(at.weekday as usize).copied().unwrap_or("")),
        _ => format!("{} {clock}", at.date()),
    }
}

pub(crate) fn relative(at: i64, now: i64) -> String {
    let delta = at - now;
    let seconds = delta.unsigned_abs();
    if seconds < 60 {
        return "now".into();
    }
    let text = span(seconds / 86_400, seconds % 86_400 / 3600, seconds % 3600 / 60);
    if delta > 0 { format!("in {text}") } else { format!("{text} ago") }
}

fn span(days: u64, hours: u64, minutes: u64) -> String {
    match (days, hours, minutes) {
        (0, 0, m) => format!("{m}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}
