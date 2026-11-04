//! Test data shared by the rules page tests.

use catalogue_core::{
    AppFeaturedThresholds, Date, DormancyThresholds, FallbackThresholds, FinishedThresholds,
    IntervalThresholds, LibraryFeaturedThresholds, MovementThresholds, NewThresholds,
    RotThresholds, Thresholds,
};

/// A thresholds file at version 3, calibrated on `calibrated`.
pub(super) fn thresholds(calibrated: Option<Date>) -> Thresholds {
    Thresholds {
        version: 3,
        calibrated,
        new: NewThresholds {
            max_age_days: 180,
            min_active_users: 2.0,
        },
        library_featured: LibraryFeaturedThresholds {
            min_active_users: 11.0,
            min_new_adopters: 9.0,
            min_former_glory: 0.5,
        },
        app_featured: AppFeaturedThresholds {
            min_signals: 2,
            binary_downloads: 884,
            homebrew_on_request_365d: 500,
            repology_families: 2,
            docker_pull_growth_90d: 10_000,
        },
        dormancy: DormancyThresholds {
            min_days: 365,
            dead_rot_signals: 2,
            dead_consecutive_checks: 2,
        },
        interval: IntervalThresholds {
            patch_gt3_major_ge1: 700,
            patch_gt3: 450,
            patch_gt0: 300,
            major_ge1: 200,
            minor_gt3: 140,
            other: 80,
            days_per_release_cap: 50,
            downloads_per_month_double: 50_000,
            active_users_double: 10.0,
        },
        rot: RotThresholds {
            unanswered_days: 270,
            maintainer_silence_days: 365,
            advisory_grace_days: 60,
            lost_glory_below: 0.5,
            build_check_max_age_days: 45,
        },
        finished: FinishedThresholds {
            min_former_glory: 0.8,
            min_new_adopters: 1.0,
            min_binary_downloads_per_release: 200,
        },
        fallback: FallbackThresholds { max_age_days: 14 },
        movement: MovementThresholds {
            window_days: 30,
            min_share: 0.25,
            min_users: 5.0,
        },
    }
}
