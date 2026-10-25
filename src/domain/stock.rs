//! Stock accounting rules for 3D-printer filament.
//!
//! The amount of filament left on the spools is measured by the printers
//! (see [`crate::domain::filament`]). Orders that are accepted but not yet
//! printed reserve their `sliced_weight_grams` on top of that measure, so
//! two queued orders cannot both count on the same filament. This module
//! owns that arithmetic so both the in-memory tests and the SQLite
//! implementation share a single source of truth.

use super::errors::DomainError;

/// Filament still free for new orders: what the printers measure minus
/// what queued orders already reserve. Not clamped: a negative result
/// means the queue already needs more than what is loaded.
pub fn available_weight(measured_remaining_grams: f64, reserved_grams: f64) -> f64 {
    measured_remaining_grams - reserved_grams
}

/// Verifies that a newly requested weight fits in the free stock of a
/// material. Returns `Ok(())` when `requested_grams` stays within
/// `measured_remaining_grams - reserved_grams`, otherwise returns a
/// [`DomainError::InsufficientStock`] carrying the numbers the admin
/// needs to diagnose the overdraft.
///
/// `reserved_grams` must already exclude the order currently being
/// updated so we never double-count its own previous weight.
pub fn check_sufficient(
    material_id: i64,
    measured_remaining_grams: f64,
    reserved_grams: f64,
    requested_grams: f64,
) -> Result<(), DomainError> {
    let available = available_weight(measured_remaining_grams, reserved_grams);
    if requested_grams > available {
        return Err(DomainError::InsufficientStock {
            material_id,
            requested_grams,
            available_grams: available,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_weight_nominal() {
        assert!((available_weight(1000.0, 300.0) - 700.0).abs() < f64::EPSILON);
    }

    #[test]
    fn available_weight_nothing_reserved() {
        assert!((available_weight(1000.0, 0.0) - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn available_weight_fully_reserved() {
        assert!(available_weight(1000.0, 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn available_weight_overdraft_is_negative() {
        // The function does not clamp; callers decide what to do with it.
        assert!(available_weight(1000.0, 1200.0) < 0.0);
    }

    #[test]
    fn check_sufficient_accepts_when_fits() {
        assert!(check_sufficient(1, 1000.0, 300.0, 500.0).is_ok());
    }

    #[test]
    fn check_sufficient_accepts_exact_fit() {
        assert!(check_sufficient(1, 1000.0, 700.0, 300.0).is_ok());
    }

    #[test]
    fn check_sufficient_rejects_overdraft() {
        let err = check_sufficient(42, 1000.0, 700.0, 500.0).unwrap_err();
        match err {
            DomainError::InsufficientStock {
                material_id,
                requested_grams,
                available_grams,
            } => {
                assert_eq!(material_id, 42);
                assert!((requested_grams - 500.0).abs() < f64::EPSILON);
                assert!((available_grams - 300.0).abs() < f64::EPSILON);
            }
            other => panic!("expected InsufficientStock, got {other:?}"),
        }
    }

    #[test]
    fn check_sufficient_rejects_when_already_overdrawn() {
        let err = check_sufficient(1, 1000.0, 1200.0, 10.0).unwrap_err();
        assert!(matches!(err, DomainError::InsufficientStock { .. }));
    }
}
