//! Timestamps without a date library: RFC 3339 in UTC.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

/// (year, month 1-12, day 1-31) for days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn seconds_part(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{y}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// `2026-11-07T06:18:05Z`
pub fn rfc3339(secs: u64) -> String {
    let mut s = seconds_part(secs);
    s.push('Z');
    s
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

thread_local! {
    static CACHE: RefCell<(u64, String)> = const { RefCell::new((u64::MAX, String::new())) };
}

/// Append `2026-11-07T06:18:05.123Z` for `t`; the date part is formatted once per second
/// per thread.
pub fn write_rfc3339_millis(out: &mut Vec<u8>, t: SystemTime) {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    CACHE.with_borrow_mut(|c| {
        if c.0 != secs {
            *c = (secs, seconds_part(secs));
        }
        out.extend_from_slice(c.1.as_bytes());
    });
    let ms = d.subsec_millis();
    out.extend_from_slice(&[
        b'.',
        b'0' + (ms / 100) as u8,
        b'0' + (ms / 10 % 10) as u8,
        b'0' + (ms % 10) as u8,
        b'Z',
    ]);
}

/// `2026-11-07T06:18:05.123Z` for `t`.
pub fn rfc3339_millis(t: SystemTime) -> String {
    let mut out = Vec::with_capacity(24);
    write_rfc3339_millis(&mut out, t);
    String::from_utf8(out).expect("ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_794_032_285), "2026-11-07T06:18:05Z");
        // leap day
        assert_eq!(rfc3339(1_709_164_800), "2024-02-29T00:00:00Z");
        let t = UNIX_EPOCH + Duration::from_millis(1_794_032_285_042);
        assert_eq!(rfc3339_millis(t), "2026-11-07T06:18:05.042Z");
        assert_eq!(
            rfc3339_millis(t + Duration::from_millis(1958)),
            "2026-11-07T06:18:07.000Z"
        );
    }
}
