//! Reminders (#109): documents with a due date coming up, for the
//! "Coming up" list, the Dock badge and the notifications.
//!
//! Everything a reminder needs lives in the document (its due date, how
//! early to remind, done and "remind me later", see [`crate::meta::DocMeta`]),
//! so every computer and the phone show the same list. Only how to notify is
//! a setting of each computer ([`ReminderSettings`]).

use serde::{Deserialize, Serialize};

use crate::i18n::{tr, trf};

// MARK: - Days

/// A calendar day, counted from 1970-01-01.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(i64);

impl Day {
    /// `YYYY-MM-DD`.
    pub fn parse(s: &str) -> Option<Day> {
        if !crate::meta::is_iso_date(s) {
            return None;
        }
        let y: i64 = s[0..4].parse().ok()?;
        let m: u32 = s[5..7].parse().ok()?;
        let d: u32 = s[8..10].parse().ok()?;
        if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
            return None;
        }
        Some(Day::from_ymd(y, m, d))
    }

    pub fn from_ymd(y: i64, m: u32, d: u32) -> Day {
        // Howard Hinnant's days_from_civil.
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m as i64 + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Day(era * 146_097 + doe - 719_468)
    }

    pub fn ymd(self) -> (i64, u32, u32) {
        let z = self.0 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
        (y, m, d)
    }

    pub fn iso(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }

    pub fn plus_days(self, n: i64) -> Day {
        Day(self.0 + n)
    }

    /// The same day `n` months later (or earlier), or the month's last day
    /// when it is shorter (Mar 31 − 1 month = Feb 28).
    pub fn plus_months(self, n: i64) -> Day {
        let (y, m, d) = self.ymd();
        let months = y * 12 + (m as i64 - 1) + n;
        let (y, m) = (months.div_euclid(12), (months.rem_euclid(12) + 1) as u32);
        Day::from_ymd(y, m, d.min(days_in_month(y, m)))
    }

    /// Days from `self` to `other`.
    pub fn until(self, other: Day) -> i64 {
        other.0 - self.0
    }

    /// Today on this computer's clock and time zone.
    pub fn today() -> Day {
        // SQLite knows the local time zone on every platform Rupert runs on.
        let local = rusqlite::Connection::open_in_memory().ok().and_then(|c| {
            c.query_row("SELECT date('now', 'localtime')", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
        });
        local.as_deref().and_then(Day::parse).unwrap_or_else(|| {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            Day(secs.div_euclid(86_400))
        })
    }
}

/// Minutes since midnight on this computer's clock.
pub fn local_minutes() -> u32 {
    rusqlite::Connection::open_in_memory()
        .ok()
        .and_then(|c| {
            c.query_row(
                "SELECT CAST(strftime('%H', 'now', 'localtime') AS INTEGER) * 60
                      + CAST(strftime('%M', 'now', 'localtime') AS INTEGER)",
                [],
                |r| r.get::<_, i64>(0),
            )
            .ok()
        })
        .map(|m| m as u32)
        .unwrap_or(0)
}

/// Seconds since 1970 (UTC) of `day` at `hour:minute` local time.
pub fn local_epoch(day: Day, hour: u32, minute: u32) -> Option<i64> {
    let c = rusqlite::Connection::open_in_memory().ok()?;
    c.query_row(
        "SELECT CAST(strftime('%s', ?1, 'utc') AS INTEGER)",
        [format!("{} {hour:02}:{minute:02}:00", day.iso())],
        |r| r.get::<_, i64>(0),
    )
    .ok()
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

// MARK: - Lead

/// How early a document shows up before its due date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lead {
    #[serde(rename = "0d")]
    OnTheDay,
    #[serde(rename = "3d")]
    Days3,
    #[serde(rename = "1w")]
    Week1,
    #[serde(rename = "2w")]
    Weeks2,
    #[serde(rename = "1m")]
    Month1,
    #[serde(rename = "3m")]
    Months3,
    #[serde(rename = "6m")]
    Months6,
    /// Never in Coming up (still found by its due date).
    #[serde(rename = "never")]
    Never,
}

impl Lead {
    pub const ALL: [Lead; 8] = [
        Lead::OnTheDay,
        Lead::Days3,
        Lead::Week1,
        Lead::Weeks2,
        Lead::Month1,
        Lead::Months3,
        Lead::Months6,
        Lead::Never,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Lead::OnTheDay => "0d",
            Lead::Days3 => "3d",
            Lead::Week1 => "1w",
            Lead::Weeks2 => "2w",
            Lead::Month1 => "1m",
            Lead::Months3 => "3m",
            Lead::Months6 => "6m",
            Lead::Never => "never",
        }
    }

    pub fn parse(s: &str) -> Option<Lead> {
        let s = s.trim();
        Lead::ALL
            .into_iter()
            .find(|l| l.code().eq_ignore_ascii_case(s))
    }

    /// The first day a document due on `due` shows up; `None` for never.
    pub fn start(self, due: Day) -> Option<Day> {
        Some(match self {
            Lead::OnTheDay => due,
            Lead::Days3 => due.plus_days(-3),
            Lead::Week1 => due.plus_days(-7),
            Lead::Weeks2 => due.plus_days(-14),
            Lead::Month1 => due.plus_months(-1),
            Lead::Months3 => due.plus_months(-3),
            Lead::Months6 => due.plus_months(-6),
            Lead::Never => return None,
        })
    }
}

// MARK: - Settings

/// How this computer reminds (Settings ▸ Reminders).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReminderSettings {
    /// The whole feature: off hides Coming up, the reminder fields in
    /// Details, notifications and the badge.
    pub enabled: bool,
    /// How early documents with an amount (bills) show up, unless the
    /// document says otherwise.
    pub bills: Lead,
    /// How early everything else (passports, contracts, warranties) does.
    pub other: Lead,
    /// Notifications from the system (they arrive even when Rupert is
    /// closed where the system can schedule them).
    pub notify: bool,
    pub notify_kind: NotifyKind,
    /// When notifications come, `HH:MM` in local time.
    pub notify_time: String,
    /// The count from Coming up on the Dock icon (macOS).
    pub dock_badge: bool,
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            bills: Lead::Week1,
            other: Lead::Months3,
            notify: false,
            notify_kind: NotifyKind::Summary,
            notify_time: "09:00".into(),
            dock_badge: false,
        }
    }
}

impl ReminderSettings {
    /// `notify_time` as hours and minutes (09:00 when it can't be read).
    pub fn time(&self) -> (u32, u32) {
        let mut parts = self.notify_time.trim().split(':');
        let h = parts
            .next()
            .and_then(|h| h.parse().ok())
            .filter(|h| *h < 24);
        let m = parts
            .next()
            .and_then(|m| m.parse().ok())
            .filter(|m| *m < 60);
        match (h, m) {
            (Some(h), Some(m)) => (h, m),
            _ => (9, 0),
        }
    }

    /// The lead a document uses: its own, else the default for its kind.
    pub fn lead_for(&self, doc: &Dated) -> Lead {
        doc.remind
            .as_deref()
            .and_then(Lead::parse)
            .unwrap_or(if doc.amount.is_some() {
                self.bills
            } else {
                self.other
            })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotifyKind {
    /// One notification a day, on days something is due soon, overdue or
    /// newly in Coming up.
    #[default]
    Summary,
    /// One for each document, when it shows up and on its due day.
    Each,
}

// MARK: - Coming up

/// A document with a due date, as the index knows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dated {
    pub source_id: String,
    pub rel_path: String,
    pub name: String,
    pub kind: crate::FileKind,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub cloud: crate::CloudState,
    pub title: Option<String>,
    pub sender: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub due_date: String,
    pub remind: Option<String>,
    pub done: Option<String>,
    pub snoozed: Option<String>,
}

impl Dated {
    /// What to call it: its title, else its file name without extension.
    pub fn label(&self) -> String {
        self.title
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| {
                std::path::Path::new(&self.name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| self.name.clone())
            })
    }
}

/// Where an item sits in Coming up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Overdue,
    /// Due within 7 days.
    Week,
    /// Due within 30 days.
    Month,
    Later,
    Done,
}

/// What Coming up shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Show {
    /// Documents whose reminder time has come (the list's normal view).
    #[default]
    Reminding,
    /// Every document with a due date that isn't done.
    All,
    /// Documents marked done, most recent due date first.
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(flatten)]
    pub doc: Dated,
    /// The lead in effect (its own or the default), as a code.
    pub lead: &'static str,
    /// Days from today to the due date (negative: overdue).
    pub days: i64,
    pub group: Group,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// Overdue or due within 7 days, and reminding: the sidebar and Dock
    /// badge.
    pub soon: usize,
    pub overdue: usize,
    /// Everything reminding now.
    pub reminding: usize,
}

fn reminding_on(doc: &Dated, lead: Lead, due: Day, day: Day) -> bool {
    doc.done.is_none()
        && lead.start(due).is_some_and(|start| start <= day)
        && !doc
            .snoozed
            .as_deref()
            .and_then(Day::parse)
            .is_some_and(|until| until > day)
}

fn group_of(days: i64) -> Group {
    match days {
        d if d < 0 => Group::Overdue,
        0..=7 => Group::Week,
        8..=30 => Group::Month,
        _ => Group::Later,
    }
}

/// The Coming up list for `today`: soonest first (done: latest first).
pub fn coming_up(docs: Vec<Dated>, s: &ReminderSettings, today: Day, show: Show) -> Vec<Item> {
    let mut items: Vec<Item> = docs
        .into_iter()
        .filter_map(|doc| {
            let due = Day::parse(&doc.due_date)?;
            let lead = s.lead_for(&doc);
            let keep = match show {
                Show::Reminding => reminding_on(&doc, lead, due, today),
                Show::All => doc.done.is_none(),
                Show::Done => doc.done.is_some(),
            };
            let days = today.until(due);
            keep.then(|| Item {
                group: if show == Show::Done {
                    Group::Done
                } else {
                    group_of(days)
                },
                lead: lead.code(),
                days,
                doc,
            })
        })
        .collect();
    items.sort_by(|a, b| {
        let order = a
            .days
            .cmp(&b.days)
            .then_with(|| a.doc.label().cmp(&b.doc.label()));
        if show == Show::Done {
            order.reverse()
        } else {
            order
        }
    });
    items
}

pub fn counts(docs: &[Dated], s: &ReminderSettings, today: Day) -> Counts {
    let mut c = Counts::default();
    for doc in docs {
        let Some(due) = Day::parse(&doc.due_date) else {
            continue;
        };
        if !reminding_on(doc, s.lead_for(doc), due, today) {
            continue;
        }
        let days = today.until(due);
        c.reminding += 1;
        if days <= 7 {
            c.soon += 1;
        }
        if days < 0 {
            c.overdue += 1;
        }
    }
    c
}

/// A count for a badge, as the window shows it: in full up to 999, then
/// 1.2K … 9.9K, 12K … 999K, 1.2M (one decimal below ten, rounded down), with
/// `decimal` as the decimal mark.
pub fn short_count(n: usize, decimal: char) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let (unit_size, unit) = if n < 1_000_000 {
        (1_000, 'K')
    } else {
        (1_000_000, 'M')
    };
    let whole = n / unit_size;
    if whole >= 10 {
        return format!("{whole}{unit}");
    }
    match n % unit_size * 10 / unit_size {
        0 => format!("{whole}{unit}"),
        tenth => format!("{whole}{decimal}{tenth}{unit}"),
    }
}

// MARK: - Notifications

/// A notification to hand to the system ahead of time, delivered on `day`
/// at the time from the settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Planned {
    /// Stable, so planning again replaces rather than repeats it.
    pub id: String,
    pub day: String,
    pub title: String,
    pub body: String,
    /// The document, for one-per-document notifications (their buttons:
    /// Mark done, Show, Tomorrow).
    pub source_id: Option<String>,
    pub rel_path: Option<String>,
}

/// How far ahead notifications are planned. Rupert plans again on every
/// start and whenever documents or settings change.
pub const PLAN_DAYS: i64 = 30;
/// The system keeps a limited number per app (64 on Apple systems).
pub const PLAN_MAX: usize = 60;

/// The notifications for the next [`PLAN_DAYS`] days, as things stand.
/// Today's come only when `time_passed` is false (the hour hasn't come yet).
pub fn plan(docs: &[Dated], s: &ReminderSettings, today: Day, time_passed: bool) -> Vec<Planned> {
    if !s.enabled || !s.notify {
        return Vec::new();
    }
    let first = if time_passed {
        today.plus_days(1)
    } else {
        today
    };
    let last = today.plus_days(PLAN_DAYS);
    let mut out = match s.notify_kind {
        NotifyKind::Each => plan_each(docs, s, first, last),
        NotifyKind::Summary => plan_summary(docs, s, first, last),
    };
    out.sort_by(|a, b| a.day.cmp(&b.day).then_with(|| a.id.cmp(&b.id)));
    out.truncate(PLAN_MAX);
    out
}

/// The document a one-per-document notification is about, from its id
/// (`doc|<day>|<source id>|<path>`), for its buttons.
pub fn doc_of(id: &str) -> Option<(String, String)> {
    let mut parts = id.splitn(4, '|');
    (parts.next()? == "doc").then_some(())?;
    parts.next()?;
    Some((parts.next()?.to_string(), parts.next()?.to_string()))
}

fn due_text(days: i64) -> String {
    match days {
        d if d < -1 => trf("Overdue by {0} days", &[&(-d).to_string()]),
        -1 => tr("Overdue since yesterday").to_string(),
        0 => tr("Due today").to_string(),
        1 => tr("Due tomorrow").to_string(),
        d => trf("Due in {0} days", &[&d.to_string()]),
    }
}

fn plan_each(docs: &[Dated], s: &ReminderSettings, first: Day, last: Day) -> Vec<Planned> {
    let mut out = Vec::new();
    for doc in docs.iter().filter(|d| d.done.is_none()) {
        let Some(due) = Day::parse(&doc.due_date) else {
            continue;
        };
        let lead = s.lead_for(doc);
        let Some(start) = lead.start(due) else {
            continue;
        };
        let snoozed = doc.snoozed.as_deref().and_then(Day::parse);
        // When it shows up, when a "remind me later" ends, and on the day.
        let mut days = vec![start, due];
        days.extend(snoozed);
        days.sort();
        days.dedup();
        for day in days {
            if day < first || day > last || snoozed.is_some_and(|until| day < until) {
                continue;
            }
            let mut body = due_text(day.until(due));
            if let Some(from) = &doc.sender {
                body = format!("{body} · {from}");
            }
            out.push(Planned {
                id: format!("doc|{}|{}|{}", day.iso(), doc.source_id, doc.rel_path),
                day: day.iso(),
                title: doc.label(),
                body,
                source_id: Some(doc.source_id.clone()),
                rel_path: Some(doc.rel_path.clone()),
            });
        }
    }
    out
}

fn plan_summary(docs: &[Dated], s: &ReminderSettings, first: Day, last: Day) -> Vec<Planned> {
    let mut out = Vec::new();
    let mut day = first;
    while day <= last {
        let mut overdue = 0;
        let mut week = 0;
        let mut names: Vec<(i64, String)> = Vec::new();
        for doc in docs {
            let Some(due) = Day::parse(&doc.due_date) else {
                continue;
            };
            let lead = s.lead_for(doc);
            if !reminding_on(doc, lead, due, day) {
                continue;
            }
            let days = day.until(due);
            let new = lead.start(due) == Some(day)
                || doc.snoozed.as_deref().and_then(Day::parse) == Some(day);
            if days < 0 {
                overdue += 1;
            } else if days <= 7 {
                week += 1;
            } else if !new {
                continue;
            }
            names.push((days, doc.label()));
        }
        if !names.is_empty() {
            names.sort();
            let mut parts = Vec::new();
            if overdue > 0 {
                parts.push(trf("Overdue: {0}", &[&overdue.to_string()]));
            }
            if week > 0 {
                parts.push(trf("Due within a week: {0}", &[&week.to_string()]));
            }
            let mut list: Vec<String> = names.iter().take(3).map(|(_, n)| n.clone()).collect();
            if names.len() > 3 {
                list.push(trf("{0} more", &[&(names.len() - 3).to_string()]));
            }
            let body = if parts.is_empty() {
                list.join(", ")
            } else {
                format!("{}: {}", parts.join(" · "), list.join(", "))
            };
            out.push(Planned {
                id: format!("summary|{}", day.iso()),
                day: day.iso(),
                title: tr("Coming up").to_string(),
                body,
                source_id: None,
                rel_path: None,
            });
        }
        day = day.plus_days(1);
    }
    out
}

// MARK: - Calendar

/// An iCalendar file with one all-day event on the due date and an alarm
/// at 9:00 when the reminder time starts, for "Add to Calendar…".
pub fn calendar_event(doc: &Dated, lead: Lead, stamp_utc: &str) -> Option<String> {
    let due = Day::parse(&doc.due_date)?;
    let compact = |d: Day| d.iso().replace('-', "");
    let mut summary = doc.label();
    if let Some(from) = &doc.sender
        && !summary.contains(from.as_str())
    {
        summary = format!("{summary} ({from})");
    }
    let mut ics = String::from(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Rupert//Reminders//EN\r\nCALSCALE:GREGORIAN\r\nBEGIN:VEVENT\r\n",
    );
    let uid = format!("{}-{}@rupert", doc.due_date, short_hash(&doc.rel_path));
    ics.push_str(&format!("UID:{uid}\r\nDTSTAMP:{stamp_utc}\r\n"));
    ics.push_str(&format!(
        "DTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\n",
        compact(due),
        compact(due.plus_days(1))
    ));
    ics.push_str(&fold(&format!("SUMMARY:{}", ics_text(&summary))));
    let mut note = due_label(doc);
    note.push_str(&format!("\n{}", doc.rel_path));
    ics.push_str(&fold(&format!("DESCRIPTION:{}", ics_text(&note))));
    if let Some(start) = lead.start(due) {
        // An all-day event starts at midnight: 9:00 on the start day.
        let hours = start.until(due) * 24 - 9;
        let trigger = if hours > 0 {
            format!("-PT{hours}H")
        } else {
            format!("PT{}H", -hours)
        };
        ics.push_str(&format!(
            "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:{trigger}\r\n"
        ));
        ics.push_str(&fold(&format!("DESCRIPTION:{}", ics_text(&summary))));
        ics.push_str("END:VALARM\r\n");
    }
    ics.push_str("END:VEVENT\r\nEND:VCALENDAR\r\n");
    Some(ics)
}

fn due_label(doc: &Dated) -> String {
    match (&doc.amount, &doc.currency) {
        (Some(a), Some(c)) => format!("{a} {c}"),
        (Some(a), None) => a.clone(),
        _ => String::new(),
    }
}

fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes())
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Escapes text for an iCalendar value.
fn ics_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace('\n', "\\n")
}

/// Folds a content line at 75 bytes (RFC 5545), on character boundaries.
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut len = 0;
    for ch in line.chars() {
        let n = ch.len_utf8();
        if len + n > 75 {
            out.push_str("\r\n ");
            len = 1;
        }
        out.push(ch);
        len += n;
    }
    out.push_str("\r\n");
    out
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn doc(name: &str, due: &str, amount: Option<&str>) -> Dated {
        Dated {
            source_id: "s".into(),
            rel_path: format!("Bills/{name}.pdf"),
            name: format!("{name}.pdf"),
            kind: crate::FileKind::Pdf,
            size: 1,
            modified_ms: None,
            cloud: crate::CloudState::Local,
            title: None,
            sender: None,
            amount: amount.map(Into::into),
            currency: amount.map(|_| "USD".into()),
            due_date: due.into(),
            remind: None,
            done: None,
            snoozed: None,
        }
    }

    #[test]
    fn days_and_months() {
        assert_eq!(day("1970-01-01").0, 0);
        for s in ["2024-02-29", "2026-11-27", "1999-12-31", "2100-03-01"] {
            assert_eq!(day(s).iso(), s);
        }
        assert!(Day::parse("2026-02-29").is_none());
        assert_eq!(day("2026-03-31").plus_months(-1).iso(), "2026-02-28");
        assert_eq!(day("2026-01-15").plus_months(-3).iso(), "2025-10-15");
        assert_eq!(day("2026-11-27").plus_days(5).iso(), "2026-12-02");
        assert_eq!(day("2026-11-27").until(day("2026-12-02")), 5);
    }

    #[test]
    fn leads_parse_and_start() {
        for l in Lead::ALL {
            assert_eq!(Lead::parse(l.code()), Some(l));
        }
        assert_eq!(Lead::parse("2y"), None);
        let due = day("2027-04-12");
        assert_eq!(Lead::Months3.start(due), Some(day("2027-01-12")));
        assert_eq!(Lead::Week1.start(due), Some(day("2027-04-05")));
        assert_eq!(Lead::Never.start(due), None);
    }

    #[test]
    fn coming_up_groups_counts_and_hides_done_and_snoozed() {
        let s = ReminderSettings::default();
        let today = day("2026-11-27");
        let mut passport = doc("Passport", "2027-02-12", None); // 3 months: from Nov 12
        passport.title = Some("Passport".into());
        let mut paid = doc("Water", "2026-11-28", Some("30.00"));
        paid.done = Some("2026-11-20".into());
        let mut later = doc("Gym", "2026-11-29", Some("20.00"));
        later.snoozed = Some("2026-11-28".into());
        let mut never = doc("Lease", "2026-12-01", None);
        never.remind = Some("never".into());
        let docs = vec![
            doc("Tax", "2026-11-20", Some("612.00")),
            doc("Power", "2026-11-30", Some("84.20")),
            doc("Boat", "2026-12-15", Some("1180.00")), // 1 week: not yet
            passport,
            paid,
            later,
            never,
        ];
        let list = coming_up(docs.clone(), &s, today, Show::Reminding);
        let names: Vec<_> = list.iter().map(|i| (i.doc.label(), i.group)).collect();
        assert_eq!(
            names,
            vec![
                ("Tax".to_string(), Group::Overdue),
                ("Power".to_string(), Group::Week),
                ("Passport".to_string(), Group::Later),
            ]
        );
        let c = counts(&docs, &s, today);
        assert_eq!((c.soon, c.overdue, c.reminding), (2, 1, 3));
        // The snoozed one is back the next day.
        let tomorrow = coming_up(docs.clone(), &s, today.plus_days(1), Show::Reminding);
        assert!(tomorrow.iter().any(|i| i.doc.label() == "Gym"));
        assert_eq!(coming_up(docs.clone(), &s, today, Show::All).len(), 6);
        let done = coming_up(docs, &s, today, Show::Done);
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].group, Group::Done);
    }

    #[test]
    fn summary_and_each_plans() {
        let today = day("2026-11-27");
        let mut s = ReminderSettings::default();
        let docs = vec![
            doc("Power", "2026-11-30", Some("84.20")),
            doc("Boat", "2026-12-15", Some("1180.00")),
        ];
        assert!(plan(&docs, &s, today, false).is_empty(), "off at first");
        s.notify = true;
        let summary = plan(&docs, &s, today, false);
        // Power is due within a week from today on; Boat from Dec 8.
        assert_eq!(summary[0].day, "2026-11-27");
        assert!(summary[0].body.contains("Power"));
        assert!(summary.iter().all(|p| p.id.starts_with("summary|")));
        assert_eq!(doc_of(&summary[0].id), None);
        // After 9:00 today's is left out.
        assert_eq!(plan(&docs, &s, today, true)[0].day, "2026-11-28");
        s.notify_kind = NotifyKind::Each;
        let each = plan(&docs, &s, today, false);
        let days: Vec<_> = each
            .iter()
            .map(|p| (p.title.as_str(), p.day.as_str()))
            .collect();
        assert_eq!(
            days,
            vec![
                ("Power", "2026-11-30"),
                ("Boat", "2026-12-08"),
                ("Boat", "2026-12-15"),
            ]
        );
        assert_eq!(
            doc_of(&each[0].id),
            Some(("s".to_string(), "Bills/Power.pdf".to_string()))
        );
        s.enabled = false;
        assert!(plan(&docs, &s, today, false).is_empty());
    }

    #[test]
    fn short_counts() {
        assert_eq!(short_count(7, '.'), "7");
        assert_eq!(short_count(999, '.'), "999");
        assert_eq!(short_count(1_000, '.'), "1K");
        assert_eq!(short_count(1_250, '.'), "1.2K");
        assert_eq!(short_count(1_999, ','), "1,9K");
        assert_eq!(short_count(9_999, '.'), "9.9K");
        assert_eq!(short_count(12_345, '.'), "12K");
        assert_eq!(short_count(2_500_000, '.'), "2.5M");
    }

    #[test]
    fn calendar_file() {
        let mut d = doc("Power", "2026-11-30", Some("84.20"));
        d.sender = Some("City Power".into());
        let ics = calendar_event(&d, Lead::Week1, "20261127T090000Z").unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20261130\r\n"));
        assert!(ics.contains("DTEND;VALUE=DATE:20261201\r\n"));
        assert!(ics.contains("SUMMARY:Power (City Power)\r\n"));
        assert!(ics.contains("TRIGGER:-PT159H\r\n"), "{ics}");
        assert!(ics.lines().all(|l| l.len() <= 75));
        let on_day = calendar_event(&d, Lead::OnTheDay, "20261127T090000Z").unwrap();
        assert!(on_day.contains("TRIGGER:PT9H\r\n"));
    }
}
