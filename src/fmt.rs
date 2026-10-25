const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];

/// The value scaled to its unit, and the unit's index into `B K M G T P`.
fn scale(bytes: u64) -> (f64, usize) {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    (value, unit)
}

/// Index of the unit [`human_bytes`] picks: 0 for B, 1 for K, ... 5 for P.
pub fn byte_unit(bytes: u64) -> usize {
    scale(bytes).1
}

/// Formats a byte count glances-style: `512B`, `1.5K`, `3.2G`.
pub fn human_bytes(bytes: u64) -> String {
    let (value, unit) = scale(bytes);
    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

pub fn human_rate(bytes_per_sec: u64) -> String {
    format!("{}/s", human_bytes(bytes_per_sec))
}

/// `3d 04:05:06`, or `04:05:06` below a day.
pub fn format_uptime(secs: u64) -> String {
    let (d, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    if d > 0 {
        format!("{d}d {h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes() {
        assert_eq!(human_bytes(0), "0B");
        assert_eq!(human_bytes(1023), "1023B");
        assert_eq!(human_bytes(1536), "1.5K");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0G");
        assert_eq!(human_rate(2048), "2.0K/s");
    }

    #[test]
    fn units() {
        assert_eq!(byte_unit(1023), 0);
        assert_eq!(byte_unit(1024), 1);
        assert_eq!(byte_unit(3 * 1024 * 1024 * 1024), 3);
        assert_eq!(byte_unit(u64::MAX), 5);
    }

    #[test]
    fn uptime() {
        assert_eq!(format_uptime(59), "00:00:59");
        assert_eq!(format_uptime(3 * 86_400 + 4 * 3600 + 5 * 60 + 6), "3d 04:05:06");
    }
}
