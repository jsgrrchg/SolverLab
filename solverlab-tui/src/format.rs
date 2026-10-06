//! Display formatting shared by the UI, the CSV export and headless mode.

/// Port of `SimulationViewModel.formatDuration`: seconds are rounded up and
/// clamped at zero; hours are shown only when non-zero.
pub fn format_duration(secs: f64) -> String {
    let total = if secs.is_finite() {
        secs.ceil().max(0.0) as i64
    } else {
        0
    };
    let hours = total / 3600;
    let minutes = total / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours:02}:{:02}:{seconds:02}", minutes % 60)
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_matches_swift() {
        assert_eq!(format_duration(-5.0), "00:00");
        assert_eq!(format_duration(61.1), "01:02");
        assert_eq!(format_duration(3661.0), "01:01:01");
        assert_eq!(format_duration(7322.0), "02:02:02");
        assert_eq!(format_duration(3599.2), "01:00:00");
        assert_eq!(format_duration(3598.5), "59:59");
        assert_eq!(format_duration(f64::NAN), "00:00");
        assert_eq!(format_duration(f64::INFINITY), "00:00");
    }
}
