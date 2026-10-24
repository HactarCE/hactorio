//! A value is a single monomial: a coefficient plus an integer exponent per
//! base unit.

use std::collections::BTreeMap;

use hactorio_data::EntryId;

/// A base unit. Items and fluids are opaque prototype ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BaseUnit {
    Item(EntryId),
    Fluid(EntryId),
    Second,
    Joule,
    Craft,
}

/// A canonical monomial.
///
/// Zero exponents are never stored, so two values with the same coefficient
/// and units are equal regardless of how they were built.
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    coef: f64,
    units: BTreeMap<BaseUnit, i64>,
}

impl Value {
    /// A dimensionless number.
    pub fn scalar(coef: f64) -> Self {
        Value {
            coef,
            units: BTreeMap::new(),
        }
    }

    /// Builds a value, folding duplicate units and dropping zero exponents.
    pub fn new(coef: f64, units: impl IntoIterator<Item = (BaseUnit, i64)>) -> Self {
        let mut map = BTreeMap::new();
        for (unit, exponent) in units {
            *map.entry(unit).or_insert(0) += exponent;
        }
        map.retain(|_, exponent| *exponent != 0);
        Value { coef, units: map }
    }

    /// `n second`.
    pub fn seconds(n: f64) -> Self {
        Value::new(n, [(BaseUnit::Second, 1)])
    }

    /// `n minute`, folded into seconds.
    pub fn minutes(n: f64) -> Self {
        Value::new(n * 60.0, [(BaseUnit::Second, 1)])
    }

    /// `n hour`, folded into seconds.
    pub fn hours(n: f64) -> Self {
        Value::new(n * 3600.0, [(BaseUnit::Second, 1)])
    }

    pub fn joules(n: f64) -> Self {
        Value::new(n, [(BaseUnit::Joule, 1)])
    }

    /// `n watt`, folded into `joule / second`.
    pub fn watts(n: f64) -> Self {
        Value::new(n, [(BaseUnit::Joule, 1), (BaseUnit::Second, -1)])
    }

    pub fn crafts(n: f64) -> Self {
        Value::new(n, [(BaseUnit::Craft, 1)])
    }

    pub fn coef(&self) -> f64 {
        self.coef
    }

    pub fn units(&self) -> &BTreeMap<BaseUnit, i64> {
        &self.units
    }

    pub fn exponent(&self, unit: BaseUnit) -> i64 {
        self.units.get(&unit).copied().unwrap_or(0)
    }

    pub fn is_dimensionless(&self) -> bool {
        self.units.is_empty()
    }
}
