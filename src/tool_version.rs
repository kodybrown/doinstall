//! Build identity for humans; Cargo and release tags retain their SemVer core.
#![allow(dead_code)]
use std::time::{SystemTime, UNIX_EPOCH};

pub fn timestamp(seconds: u64) -> (String, String) {
  let mut days = seconds / 86400;
  let mut year = 1970;
  let leap = |y| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
  while days >= if leap(year) { 366 } else { 365 } {
    days -= if leap(year) { 366 } else { 365 };
    year += 1;
  }
  let mut month = 1;
  for count in [
    31,
    if leap(year) { 29 } else { 28 },
    31,
    30,
    31,
    30,
    31,
    31,
    30,
    31,
    30,
    31,
  ] {
    if days < count {
      break;
    }
    days -= count;
    month += 1;
  }
  let day = days + 1;
  let hour = seconds % 86400 / 3600;
  let minute = seconds % 3600 / 60;
  let second = seconds % 60;
  (
    format!("{:02}{month:02}.{day:02}{hour:02}", year % 100),
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
  )
}
pub fn now() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("Clock predates Unix epoch")
    .as_secs()
}
pub fn print(arguments: &[String], name: &str) -> bool {
  if arguments == ["--version"] {
    println!(
      "{name} {} (built {})",
      option_env!("DOINSTALL_DISPLAY_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
      option_env!("DOINSTALL_BUILD_UTC").unwrap_or("unknown")
    );
    true
  } else {
    false
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn utc_dates_include_leap_days_and_year_rollover() {
    assert_eq!(super::timestamp(0), ("7001.0100".into(), "1970-01-01T00:00:00Z".into()));
    assert_eq!(super::timestamp(1709164800).1, "2024-02-29T00:00:00Z");
    assert_eq!(super::timestamp(1704067200).1, "2024-01-01T00:00:00Z");
  }
}
