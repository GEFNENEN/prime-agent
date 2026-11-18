//! The `yy/mm/dd HH:MM:SS.mmm` transcript clock, kept at the OS boundary
//! so every TUI consumer shares one local-time conversion.
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
#[derive(Clone, Copy, Default)]
#[repr(C)]
struct SystemTimeParts {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[cfg(windows)]
extern "system" {
    fn SystemTimeToTzSpecificLocalTime(
        time_zone: *const std::ffi::c_void,
        universal: *const SystemTimeParts,
        local: *mut SystemTimeParts,
    ) -> i32;
}

/// The local civil fields at one instant, or `None` when the OS cannot
/// resolve that timestamp (the caller keeps the output undecorated then).
#[must_use]
pub fn local_time_parts(unix_ms: u64) -> Option<(i32, u32, u32, u32, u32, u32, u32)> {
    #[cfg(windows)]
    {
        let seconds = i64::try_from(unix_ms / 1000).ok()?;
        let utc = civil_from_days(seconds)?;
        let utc = SystemTimeParts {
            year: u16::try_from(utc.0).ok()?,
            month: u16::try_from(utc.1).ok()?,
            day: u16::try_from(utc.2).ok()?,
            hour: u16::try_from(seconds.rem_euclid(86_400) / 3_600).ok()?,
            minute: u16::try_from(seconds.rem_euclid(3_600) / 60).ok()?,
            second: u16::try_from(seconds.rem_euclid(60)).ok()?,
            milliseconds: u16::try_from(unix_ms % 1000).ok()?,
            ..Default::default()
        };
        let mut local = SystemTimeParts::default();
        // SAFETY: both pointers reference initialized or writable fixed-size
        // structs for the duration of the synchronous Win32 call; the null
        // timezone selects the machine's current zone and its historic rules.
        unsafe {
            (SystemTimeToTzSpecificLocalTime(
                std::ptr::null(),
                std::ptr::from_ref(&utc),
                std::ptr::from_mut(&mut local),
            ) != 0)
                .then_some(())?;
        }
        Some((
            i32::from(local.year),
            u32::from(local.month),
            u32::from(local.day),
            u32::from(local.hour),
            u32::from(local.minute),
            u32::from(local.second),
            u32::from(local.milliseconds),
        ))
    }
    #[cfg(unix)]
    {
        let timestamp = libc::time_t::try_from(unix_ms / 1000).ok()?;
        let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
        // SAFETY: `timestamp` is a valid value and `local` owns writable
        // `tm` storage; localtime_r uses no shared output buffer.
        unsafe {
            (!libc::localtime_r(&timestamp, local.as_mut_ptr()).is_null()).then_some(())?;
            let local = local.assume_init();
            Some((
                local.tm_year.checked_add(1900)?,
                u32::try_from(local.tm_mon.checked_add(1)?).ok()?,
                u32::try_from(local.tm_mday).ok()?,
                u32::try_from(local.tm_hour).ok()?,
                u32::try_from(local.tm_min).ok()?,
                u32::try_from(local.tm_sec).ok()?,
                u32::try_from(unix_ms % 1000).ok()?,
            ))
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = unix_ms;
        None
    }
}

#[must_use]
pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The UTC civil date used only to seed Windows' UTC -> local conversion.
#[cfg(windows)]
fn civil_from_days(seconds: i64) -> Option<(i32, u32, u32)> {
    let days = seconds.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    Some((
        i32::try_from(year).ok()?,
        u32::try_from(month).ok()?,
        u32::try_from(day).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_known_instant_matches_the_operating_system_zone() {
        // 2026-11-18 02:26:54.224 UTC. This checks calendar and millisecond
        // plumbing, not a hard-coded host timezone.
        let unix_ms = 1_794_966_414_224;
        let parts = local_time_parts(unix_ms).expect("local time");
        assert_eq!(parts.6, 224);
        assert!(parts.0 >= 2026);
        assert!((1..=12).contains(&parts.1));
        assert!((1..=31).contains(&parts.2));
        assert!(parts.3 < 24 && parts.4 < 60 && parts.5 < 61);
    }
}
