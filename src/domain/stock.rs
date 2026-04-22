//! Stock accounting rules for 3D-printer filament spools.
//!
//! A spool has a fixed total weight. Each non-cancelled order referencing
//! a material consumes part of that weight (the `sliced_weight_grams` the
//! admin enters once the job has been sliced). Remaining stock is a pure
//! function of those two inputs; this module owns that arithmetic so both
//! the in-memory tests and the SQLite implementation share a single
//! source of truth.

use super::errors::DomainError;

/// Weight still available on a spool after subtracting what running
/// orders already consume. Never returns a negative value clamped to
/// zero — the caller is expected to treat negative results as overdraft.
pub fn remaining_weight(spool_weight_grams: f64, consumed_grams: f64) -> f64
{
    spool_weight_grams - consumed_grams
}

/// Verifies that a newly requested weight fits in the remaining stock
/// for a material. Returns `Ok(())` when the sum of `already_consumed`
/// and `requested` stays within `spool_weight_grams`, otherwise returns
/// a [`DomainError::InsufficientStock`] carrying the numbers the admin
/// needs to diagnose the overdraft.
///
/// `already_consumed` must already exclude the order currently being
/// updated so we never double-count its own previous weight.
pub fn check_sufficient(
    material_id: i64,
    spool_weight_grams: f64,
    already_consumed_grams: f64,
    requested_grams: f64,
) -> Result<(), DomainError>
{
    let available = remaining_weight(spool_weight_grams, already_consumed_grams);
    if requested_grams > available
    {
        return Err(DomainError::InsufficientStock
        {
            material_id,
            requested_grams,
            available_grams: available,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn remaining_weight_nominal()
    {
        assert!((remaining_weight(1000.0, 300.0) - 700.0).abs() < f64::EPSILON);
    }

    #[test]
    fn remaining_weight_zero_consumed()
    {
        assert!((remaining_weight(1000.0, 0.0) - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn remaining_weight_fully_consumed()
    {
        assert!(remaining_weight(1000.0, 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn remaining_weight_overdraft_is_negative()
    {
        // The function does not clamp; callers decide what to do with it.
        assert!(remaining_weight(1000.0, 1200.0) < 0.0);
    }

    #[test]
    fn check_sufficient_accepts_when_fits()
    {
        assert!(check_sufficient(1, 1000.0, 300.0, 500.0).is_ok());
    }

    #[test]
    fn check_sufficient_accepts_exact_fit()
    {
        assert!(check_sufficient(1, 1000.0, 700.0, 300.0).is_ok());
    }

    #[test]
    fn check_sufficient_rejects_overdraft()
    {
        let err = check_sufficient(42, 1000.0, 700.0, 500.0).unwrap_err();
        match err
        {
            DomainError::InsufficientStock
            {
                material_id,
                requested_grams,
                available_grams,
            } =>
            {
                assert_eq!(material_id, 42);
                assert!((requested_grams - 500.0).abs() < f64::EPSILON);
                assert!((available_grams - 300.0).abs() < f64::EPSILON);
            }
            other => panic!("expected InsufficientStock, got {other:?}"),
        }
    }

    #[test]
    fn check_sufficient_rejects_when_already_overdrawn()
    {
        let err = check_sufficient(1, 1000.0, 1200.0, 10.0).unwrap_err();
        assert!(matches!(err, DomainError::InsufficientStock { .. }));
    }
}
