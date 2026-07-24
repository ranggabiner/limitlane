use crate::domain::types::{AccountAvailability, EnforcementType, UsageSnapshot};

pub fn evaluate_limiting_window(snapshot: &mut UsageSnapshot) {
    let hard_windows: Vec<_> = snapshot
        .windows
        .iter()
        .filter(|w| w.enforcement == EnforcementType::Hard)
        .collect();

    let candidate_windows = if hard_windows.is_empty() {
        snapshot.windows.iter().collect::<Vec<_>>()
    } else {
        hard_windows
    };

    let max_window = candidate_windows
        .into_iter()
        .filter_map(|w| w.used_percent.map(|pct| (pct, w)))
        .max_by(|(pct_a, _), (pct_b, _)| pct_a.partial_cmp(pct_b).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((max_pct, window)) = max_window {
        snapshot.limiting_window_id = Some(window.id.clone());
        snapshot.effective_used_percent = Some(max_pct);

        if max_pct >= 100.0 {
            snapshot.hard_limit_reached = true;
        }
    } else {
        snapshot.limiting_window_id = None;
        snapshot.effective_used_percent = None;
    }

    let any_hard_exhausted = snapshot
        .windows
        .iter()
        .filter(|w| w.enforcement == EnforcementType::Hard)
        .any(|w| {
            w.used_percent.map_or(false, |pct| pct >= 100.0)
                || w.remaining_percent.map_or(false, |rem| rem <= 0.0)
        });

    if any_hard_exhausted {
        snapshot.hard_limit_reached = true;
    }

    if snapshot.hard_limit_reached {
        let has_overflow_credits = match &snapshot.additional_usage {
            Some(add) => add.enabled && add.exhausted != Some(true),
            None => false,
        };

        if has_overflow_credits {
            snapshot.availability = AccountAvailability::AvailablePaidOverflow;
        } else {
            snapshot.availability = AccountAvailability::Limited;
        }
    } else {
        snapshot.availability = AccountAvailability::AvailableIncluded;
    }
}
