//! A value is a single monomial: a coefficient plus an integer exponent per
//! base unit.

use std::collections::BTreeMap;

use hactorio_data::EntryId;
use thiserror::Error;

/// A base unit. Items and fluids are opaque prototype ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BaseUnit {
    Item(EntryId),
    Fluid(EntryId),
    Second,
    Joule,
    Craft,
}

/// Why a value operation is not defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ValueError {
    #[error("cannot add or subtract values with different units")]
    IncompatibleUnits,
    #[error("an exponent must be dimensionless")]
    DimensionalExponent,
    #[error("a value with units cannot be raised to a fractional power")]
    FractionalExponent,
    #[error("exponent is out of range")]
    ExponentOverflow,
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

    pub fn with_unit(mut self, unit: BaseUnit, exponent: i64) -> Self {
        if exponent == 0 {
            self.units.remove(&unit);
        } else {
            self.units.insert(unit, exponent);
        }
        self
    }

    pub fn mul(&self, other: &Value) -> Value {
        let mut units = self.units.clone();
        for (unit, exponent) in &other.units {
            *units.entry(*unit).or_insert(0) += exponent;
        }
        units.retain(|_, exponent| *exponent != 0);
        Value {
            coef: self.coef * other.coef,
            units,
        }
    }

    pub fn div(&self, other: &Value) -> Value {
        let mut units = self.units.clone();
        for (unit, exponent) in &other.units {
            *units.entry(*unit).or_insert(0) -= exponent;
        }
        units.retain(|_, exponent| *exponent != 0);
        Value {
            coef: self.coef / other.coef,
            units,
        }
    }

    pub fn add(&self, other: &Value) -> Result<Value, ValueError> {
        if self.units != other.units {
            return Err(ValueError::IncompatibleUnits);
        }
        Ok(Value {
            coef: self.coef + other.coef,
            units: self.units.clone(),
        })
    }

    pub fn sub(&self, other: &Value) -> Result<Value, ValueError> {
        if self.units != other.units {
            return Err(ValueError::IncompatibleUnits);
        }
        Ok(Value {
            coef: self.coef - other.coef,
            units: self.units.clone(),
        })
    }

    pub fn neg(&self) -> Value {
        Value {
            coef: -self.coef,
            units: self.units.clone(),
        }
    }

    /// Raises the value to a dimensionless power.
    ///
    /// A value with units requires an integer exponent.
    pub fn pow(&self, exponent: &Value) -> Result<Value, ValueError> {
        if !exponent.is_dimensionless() {
            return Err(ValueError::DimensionalExponent);
        }
        let power = exponent.coef;
        if self.units.is_empty() {
            return Ok(Value::scalar(self.coef.powf(power)));
        }
        if !power.is_finite() || power.fract() != 0.0 {
            return Err(ValueError::FractionalExponent);
        }
        let power_i64 = power as i64;
        if power_i64 as f64 != power {
            return Err(ValueError::ExponentOverflow);
        }
        let mut units = BTreeMap::new();
        for (unit, exponent) in &self.units {
            let exponent = exponent
                .checked_mul(power_i64)
                .ok_or(ValueError::ExponentOverflow)?;
            if exponent != 0 {
                units.insert(*unit, exponent);
            }
        }
        Ok(Value {
            coef: self.coef.powf(power),
            units,
        })
    }
}
