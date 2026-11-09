//! Human-friendly durations ("90s", "500ms") and sizes ("2000m", "4g") for the config file.

use std::fmt;
use std::time::Duration;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(untagged)]
enum Raw {
    Num(u64),
    Str(String),
}

/// A duration written as `<number><unit>` with unit `ms`, `s`, `m` or `h`, or a bare number of seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Raw")]
pub struct Dur(pub Duration);

impl TryFrom<Raw> for Dur {
    type Error = String;

    fn try_from(raw: Raw) -> Result<Self, String> {
        match raw {
            Raw::Num(secs) => Ok(Dur(Duration::from_secs(secs))),
            Raw::Str(s) => parse_duration(&s).map(Dur),
        }
    }
}

pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: u64 = num
        .parse()
        .map_err(|_| format!("invalid duration {s:?}: expected e.g. 90s, 500ms, 10m"))?;
    let d = match unit.trim() {
        "" | "s" => Duration::from_secs(n),
        "ms" => Duration::from_millis(n),
        "m" => Duration::from_secs(n * 60),
        "h" => Duration::from_secs(n * 3600),
        "d" => Duration::from_secs(n * 86_400),
        other => return Err(format!("invalid duration unit {other:?} in {s:?}")),
    };
    Ok(d)
}

/// A byte size written like nginx: `512k`, `2000m`, `4g`, or bytes. `0` means unlimited.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Raw")]
pub struct Size(pub Option<u64>);

impl fmt::Debug for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            None => f.write_str("unlimited"),
            Some(n) => write!(f, "{n} bytes"),
        }
    }
}

impl TryFrom<Raw> for Size {
    type Error = String;

    fn try_from(raw: Raw) -> Result<Self, String> {
        let bytes = match raw {
            Raw::Num(n) => n,
            Raw::Str(s) => parse_size(&s)?,
        };
        Ok(Size((bytes != 0).then_some(bytes)))
    }
}

pub fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: u64 = num
        .parse()
        .map_err(|_| format!("invalid size {s:?}: expected e.g. 512k, 2000m, 4g"))?;
    let mult: u64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" => 1,
        "k" => 1 << 10,
        "m" => 1 << 20,
        "g" => 1 << 30,
        other => return Err(format!("invalid size unit {other:?} in {s:?}")),
    };
    n.checked_mul(mult)
        .ok_or_else(|| format!("size {s:?} is too large"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("90").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("10m").unwrap(), Duration::from_secs(600));
        assert!(parse_duration("ten").is_err());
        assert_eq!(
            parse_duration("5d").unwrap(),
            Duration::from_secs(5 * 86_400)
        );
        assert!(parse_duration("5w").is_err());
    }

    #[test]
    fn sizes() {
        assert_eq!(parse_size("2000m").unwrap(), 2000 << 20);
        assert_eq!(parse_size("4096M").unwrap(), 4096 << 20);
        assert_eq!(parse_size("4g").unwrap(), 4 << 30);
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert!(parse_size("1t").is_err());
        let zero: Size = crate::config::yaml("0").unwrap();
        assert_eq!(zero.0, None);
    }
}
