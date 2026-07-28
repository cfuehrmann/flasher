//! Pure spaced-repetition scheduling for Flasher.
//!
//! This crate is a line-for-line port of the *scheduling rules* of the
//! old .NET backend (`backend/Flasher.Host/Handlers/Cards/CardsHandler.cs`,
//! `backend/Flasher.Host/CardsOptions.cs` with the defaults from
//! `backend/Flasher.Host/appsettings.json`). Only the arithmetic is
//! ported; the surrounding request handling differs deliberately — the
//! C# `SetState` returned the *next* due card, while `flasher-server`
//! returns the *rated* card and the frontend refetches the next card.
//! This crate contains no I/O and no database access: all times are unix
//! epoch millis (`i64`). It also hosts the shared [`format_utc_date`]
//! helper (epoch millis to `YYYY-MM-DD`, UTC) so both the server and the
//! wasm frontend can format dates without a date library.

/// Default `OkMultiplier` from the old `appsettings.json`.
pub const DEFAULT_OK_MULTIPLIER: f64 = 1.8;
/// Default `FailedMultiplier` from the old `appsettings.json`.
pub const DEFAULT_FAILED_MULTIPLIER: f64 = 0.5555;
/// Default `NewCardWaitingTime` (30 minutes) from the old `appsettings.json`.
pub const DEFAULT_NEW_CARD_WAITING_MS: i64 = 30 * 60 * 1_000;

/// Configuration of the scheduling rules (port of `CardsOptions`; the old
/// `PageSize` belongs to the groom/search slice and is not part of the SRS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrsConfig {
    /// Multiplier applied to the last interval when a card is rated ok.
    pub ok_multiplier: f64,
    /// Multiplier applied to the last interval when a card is rated failed.
    pub failed_multiplier: f64,
    /// Waiting time before a freshly created card becomes due, in millis.
    pub new_card_waiting_ms: i64,
}

impl Default for SrsConfig {
    fn default() -> Self {
        Self {
            ok_multiplier: DEFAULT_OK_MULTIPLIER,
            failed_multiplier: DEFAULT_FAILED_MULTIPLIER,
            new_card_waiting_ms: DEFAULT_NEW_CARD_WAITING_MS,
        }
    }
}

/// Port of `CardsHandler.SetState` for a failed rating:
/// `NextTime = now + (now - ChangeTime) * FailedMultiplier`.
#[must_use]
pub fn next_time_after_failed(change_time: i64, now: i64, config: &SrsConfig) -> i64 {
    reschedule(change_time, now, config.failed_multiplier)
}

/// Port of `CardsHandler.SetState` for an ok rating:
/// `NextTime = now + (now - ChangeTime) * OkMultiplier`.
#[must_use]
pub fn next_time_after_ok(change_time: i64, now: i64, config: &SrsConfig) -> i64 {
    reschedule(change_time, now, config.ok_multiplier)
}

/// Port of `CardsHandler.Create`:
/// `NextTime = now + NewCardWaitingTime`.
///
/// Note that `SetState` has no special-casing for cards in state `New` —
/// the new-card waiting time is only applied at creation time, exactly as
/// in the old backend.
#[must_use]
pub fn next_time_for_new_card(now: i64, config: &SrsConfig) -> i64 {
    now + config.new_card_waiting_ms
}

/// Formats unix epoch millis as `YYYY-MM-DD` (UTC) in pure Rust (Howard
/// Hinnant's civil-from-days algorithm) — no date library needed, so the
/// wasm frontend can use it too. Shared by server and frontend.
#[must_use]
pub fn format_utc_date(epoch_ms: i64) -> String {
    let days = epoch_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

/// Shared body of `CardsHandler.SetState`:
///
/// ```csharp
/// TimeSpan passedTime = now - card.ChangeTime;
/// NextTime = now.Add(passedTime * multiplier); // ChangeTime becomes now
/// ```
///
/// `DateTime.Add` rounds the `TimeSpan` to the nearest millisecond, which
/// [`f64::round`] reproduces. A `change_time` in the future yields a
/// negative interval, exactly like the C# code.
fn reschedule(change_time: i64, now: i64, multiplier: f64) -> i64 {
    let passed_ms = now - change_time;
    // f64 -> i64 `as` casts saturate, so extreme inputs cannot overflow.
    // Precision loss beyond 2^52 ms (~142k years of interval) is
    // irrelevant here and matches the C# double arithmetic anyway.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let interval_ms = (passed_ms as f64 * multiplier).round() as i64;
    now + interval_ms
}

#[cfg(test)]
// float_cmp: the multipliers are pinned to their exact C# values, so
// strict equality is exactly what these tests mean.
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    // ------------------------------------------------------------ defaults

    #[test]
    fn defaults_match_the_old_appsettings() {
        let config = SrsConfig::default();
        assert_eq!(config.ok_multiplier, 1.8);
        assert_eq!(config.failed_multiplier, 0.5555);
        assert_eq!(config.new_card_waiting_ms, 1_800_000);
    }

    #[test]
    fn default_constants_are_used_by_default_impl() {
        let config = SrsConfig::default();
        assert_eq!(config.ok_multiplier, DEFAULT_OK_MULTIPLIER);
        assert_eq!(config.failed_multiplier, DEFAULT_FAILED_MULTIPLIER);
        assert_eq!(config.new_card_waiting_ms, DEFAULT_NEW_CARD_WAITING_MS);
    }

    // ------------------------------------------------------------ set-ok

    #[test]
    fn ok_rating_multiplies_the_passed_time() {
        // 10_000 ms passed, interval becomes 18_000 ms.
        let next = next_time_after_ok(1_000, 11_000, &SrsConfig::default());
        assert_eq!(next, 29_000);
    }

    #[test]
    fn ok_rating_at_change_time_yields_zero_interval() {
        // Boundary: change_time == now -> passed == 0 -> next == now.
        let next = next_time_after_ok(5_000, 5_000, &SrsConfig::default());
        assert_eq!(next, 5_000);
    }

    #[test]
    fn ok_rating_rounds_to_the_nearest_millisecond() {
        // 1 ms passed * 1.8 = 1.8 ms -> rounds up to 2 (truncation would
        // give 1); pins DateTime.Add's nearest-millisecond rounding.
        let next = next_time_after_ok(0, 1, &SrsConfig::default());
        assert_eq!(next, 3);
    }

    #[test]
    fn ok_rating_with_future_change_time_yields_negative_interval() {
        // change_time 5_000, now 2_000 -> passed == -3_000 -> interval
        // -5_400 -> next = 2_000 - 5_400.
        let next = next_time_after_ok(5_000, 2_000, &SrsConfig::default());
        assert_eq!(next, -3_400);
    }

    #[test]
    fn ok_rating_uses_the_configured_multiplier() {
        let config = SrsConfig {
            ok_multiplier: 2.0,
            failed_multiplier: 0.5,
            new_card_waiting_ms: 1_000,
        };
        let next = next_time_after_ok(0, 1_000, &config);
        assert_eq!(next, 3_000);
    }

    // --------------------------------------------------------- set-failed

    #[test]
    fn failed_rating_multiplies_the_passed_time() {
        // 10_000 ms passed, interval becomes 5_555 ms.
        let next = next_time_after_failed(1_000, 11_000, &SrsConfig::default());
        assert_eq!(next, 16_555);
    }

    #[test]
    fn failed_rating_at_change_time_yields_zero_interval() {
        // Boundary: change_time == now -> passed == 0 -> next == now.
        let next = next_time_after_failed(5_000, 5_000, &SrsConfig::default());
        assert_eq!(next, 5_000);
    }

    #[test]
    fn failed_rating_rounds_to_the_nearest_millisecond() {
        // 999 ms * 0.5555 = 554.9445 ms -> rounds up to 555 (truncation
        // would give 554); pins DateTime.Add's nearest-millisecond rounding.
        let next = next_time_after_failed(0, 999, &SrsConfig::default());
        assert_eq!(next, 1_554);
    }

    #[test]
    fn failed_rating_with_future_change_time_yields_negative_interval() {
        // change_time 5_000, now 2_000 -> passed == -3_000 -> interval
        // -1_666.5 -> rounded -1_667 (nearest) -> next = 2_000 - 1_667.
        let next = next_time_after_failed(5_000, 2_000, &SrsConfig::default());
        assert_eq!(next, 333);
    }

    #[test]
    fn failed_rating_uses_the_configured_multiplier() {
        let config = SrsConfig {
            ok_multiplier: 2.0,
            failed_multiplier: 0.5,
            new_card_waiting_ms: 1_000,
        };
        let next = next_time_after_failed(0, 1_000, &config);
        assert_eq!(next, 1_500);
    }

    #[test]
    fn failed_and_ok_ratings_use_different_multipliers() {
        // Same inputs, different multiplier per function.
        let ok = next_time_after_ok(0, 10_000, &SrsConfig::default());
        let failed = next_time_after_failed(0, 10_000, &SrsConfig::default());
        assert_eq!(ok, 28_000);
        assert_eq!(failed, 15_555);
    }

    // ------------------------------------------------------------- create

    #[test]
    fn new_card_waits_the_configured_time() {
        let next = next_time_for_new_card(1_000_000, &SrsConfig::default());
        assert_eq!(next, 2_800_000);
    }

    #[test]
    fn new_card_waiting_time_is_applied_from_now_not_from_epoch() {
        let next = next_time_for_new_card(0, &SrsConfig::default());
        assert_eq!(next, 1_800_000);
    }

    #[test]
    fn new_card_uses_the_configured_waiting_time() {
        let config = SrsConfig {
            ok_multiplier: 2.0,
            failed_multiplier: 0.5,
            new_card_waiting_ms: 1_000,
        };
        let next = next_time_for_new_card(7, &config);
        assert_eq!(next, 1_007);
    }

    // ------------------------------------------------------- utc dates

    #[test]
    fn utc_date_epoch_zero_is_1970_01_01() {
        assert_eq!(format_utc_date(0), "1970-01-01");
        assert_eq!(format_utc_date(86_400_000), "1970-01-02");
    }

    #[test]
    fn utc_date_negative_values_stay_in_1969() {
        // div_euclid floors, so even -1 ms is still the previous day.
        assert_eq!(format_utc_date(-1), "1969-12-31");
        assert_eq!(format_utc_date(-86_400_000), "1969-12-31");
    }

    #[test]
    fn utc_date_leap_years() {
        // 2000 is a leap year (divisible by 400), 2024 too.
        assert_eq!(format_utc_date(951_782_400_000), "2000-02-29");
        assert_eq!(format_utc_date(1_709_164_800_000), "2024-02-29");
        // 1900 is NOT a leap year (divisible by 100, not 400): no Feb 29.
        assert_eq!(format_utc_date(-2_203_977_600_000), "1900-02-28");
        assert_eq!(format_utc_date(-2_203_891_200_000), "1900-03-01");
    }

    #[test]
    fn utc_date_year_boundaries() {
        assert_eq!(format_utc_date(1_703_980_800_000), "2023-12-31");
        assert_eq!(format_utc_date(1_704_067_200_000), "2024-01-01");
    }
}
