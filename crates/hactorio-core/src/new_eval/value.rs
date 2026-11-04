//! The value model: qualities, quantities, and their arithmetic.
//!
//! This is the MVP described by `SPEC.md` D6: the `quantity` facet plus the two
//! reference facets, and the operators over them. The remaining facets of
//! §3.1, and the process value kind of §3.4, arrive with their increments.

use std::collections::BTreeMap;
use std::fmt;

use hactorio_data::{EntryId, RecipeId};

/// Coefficients smaller than this are treated as zero (§7.2).
pub const APPROX_ZERO: f64 = 1e-9;

pub fn is_zero(value: f64) -> bool {
    value.abs() < APPROX_ZERO
}

/// A user-facing evaluation error (§8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueError(String);

impl ValueError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ValueError {}

pub type Result<T, E = ValueError> = std::result::Result<T, E>;

/// Item quality (§4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Quality {
    #[default]
    Normal,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Quality {
    pub const ALL: [Quality; 5] = [
        Quality::Normal,
        Quality::Uncommon,
        Quality::Rare,
        Quality::Epic,
        Quality::Legendary,
    ];

    /// The registry's `level` field: `0`, `1`, `2`, `3`, `5`.
    pub fn level(self) -> u32 {
        match self {
            Quality::Normal => 0,
            Quality::Uncommon => 1,
            Quality::Rare => 2,
            Quality::Epic => 3,
            Quality::Legendary => 5,
        }
    }

    /// The standard quality scaling factor, `1 + 0.3 × level` (§4.2).
    pub fn multiplier(self) -> f64 {
        1.0 + 0.3 * f64::from(self.level())
    }

    /// The canonical spelling, which is also the token id.
    pub fn name(self) -> &'static str {
        match self {
            Quality::Normal => "normal",
            Quality::Uncommon => "uncommon",
            Quality::Rare => "rare",
            Quality::Epic => "epic",
            Quality::Legendary => "legendary",
        }
    }

    /// The glyph that renders this quality, which is not a namespace entry.
    pub fn glyph(self) -> char {
        hactorio_data::quality_by_id(self.name()).map_or('\u{FFFD}', |quality| quality.glyph)
    }

    /// Every spelling of this quality, canonical first (§4.6).
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Quality::Normal => &["normal", "common", "quality-1"],
            Quality::Uncommon => &["uncommon", "quality-2"],
            Quality::Rare => &["rare", "quality-3"],
            Quality::Epic => &["epic", "quality-4"],
            Quality::Legendary => &["legendary", "quality-5"],
        }
    }

    /// Resolves a quality token id, including the §4.6 aliases.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "normal" | "common" | "quality-1" => Quality::Normal,
            "uncommon" | "quality-2" => Quality::Uncommon,
            "rare" | "quality-3" => Quality::Rare,
            "epic" | "quality-4" => Quality::Epic,
            "legendary" | "quality-5" => Quality::Legendary,
            _ => return None,
        })
    }
}

/// An item, which is a prototype id plus a concrete quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item {
    pub id: EntryId,
    pub quality: Quality,
}

/// A resource: an item or a fluid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    Item(Item),
    Fluid(EntryId),
}

impl Resource {
    pub fn id(self) -> EntryId {
        match self {
            Resource::Item(item) => item.id,
            Resource::Fluid(id) => id,
        }
    }

    /// A fluid has no quality; an item always does.
    pub fn quality(self) -> Option<Quality> {
        match self {
            Resource::Item(item) => Some(item.quality),
            Resource::Fluid(_) => None,
        }
    }

    pub fn with_quality(self, quality: Quality) -> Self {
        match self {
            Resource::Item(item) => Resource::Item(Item { quality, ..item }),
            fluid => fluid,
        }
    }
}

/// A plain number, or a coefficient per resource (§3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Amount {
    Number(f64),
    Resources(BTreeMap<Resource, f64>),
}

impl Amount {
    pub fn number(value: f64) -> Self {
        Amount::Number(value)
    }

    pub fn number_value(&self) -> Option<f64> {
        match self {
            Amount::Number(value) => Some(*value),
            Amount::Resources(_) => None,
        }
    }

    /// Removes zero terms, collapsing an emptied bundle to the number zero.
    pub fn canonicalize(self) -> Self {
        match self {
            Amount::Resources(mut terms) => {
                terms.retain(|_, coefficient| !is_zero(*coefficient));
                if terms.is_empty() {
                    Amount::Number(0.0)
                } else {
                    Amount::Resources(terms)
                }
            }
            number => number,
        }
    }

    pub fn scale(&self, factor: f64) -> Self {
        match self {
            Amount::Number(value) => Amount::Number(value * factor),
            Amount::Resources(terms) => Amount::Resources(
                terms
                    .iter()
                    .map(|(resource, coefficient)| (*resource, coefficient * factor))
                    .collect(),
            ),
        }
        .canonicalize()
    }

    pub fn negate(&self) -> Self {
        self.scale(-1.0)
    }

    /// The single term of a one-term bundle, which is what cancellation needs.
    pub fn single_term(&self) -> Option<(Resource, f64)> {
        match self {
            Amount::Resources(terms) if terms.len() == 1 => terms
                .iter()
                .next()
                .map(|(resource, coefficient)| (*resource, *coefficient)),
            _ => None,
        }
    }

    pub fn map_resources(&self, mut map: impl FnMut(Resource) -> Resource) -> Self {
        match self {
            Amount::Number(value) => Amount::Number(*value),
            Amount::Resources(terms) => Amount::Resources(
                terms
                    .iter()
                    .map(|(resource, coefficient)| (map(*resource), *coefficient))
                    .collect(),
            ),
        }
        .canonicalize()
    }
}

/// `amount · second^s · joule^j · craft^c / per` (§5.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    pub amount: Amount,
    pub per: Option<Resource>,
    pub second: i32,
    pub joule: i32,
    pub craft: i32,
}

impl Quantity {
    pub fn number(value: f64) -> Self {
        Self {
            amount: Amount::Number(value),
            per: None,
            second: 0,
            joule: 0,
            craft: 0,
        }
    }

    pub fn resource(resource: Resource) -> Self {
        Self {
            amount: Amount::Resources(BTreeMap::from([(resource, 1.0)])),
            per: None,
            second: 0,
            joule: 0,
            craft: 0,
        }
    }

    pub fn time(seconds: f64) -> Self {
        Self {
            second: 1,
            ..Self::number(seconds)
        }
    }

    pub fn is_dimensionless_number(&self) -> Option<f64> {
        if self.per.is_some() || self.second != 0 || self.joule != 0 || self.craft != 0 {
            return None;
        }
        self.amount.number_value()
    }

    fn same_units(&self, other: &Self) -> bool {
        self.per == other.per
            && self.second == other.second
            && self.joule == other.joule
            && self.craft == other.craft
    }

    pub fn add(&self, other: &Self) -> Result<Self> {
        if !self.same_units(other) {
            return Err(ValueError::new("quantities have different units"));
        }
        let amount = match (&self.amount, &other.amount) {
            (Amount::Number(a), Amount::Number(b)) => Amount::Number(a + b),
            (Amount::Resources(a), Amount::Resources(b)) => {
                let mut terms = a.clone();
                for (resource, coefficient) in b {
                    *terms.entry(*resource).or_insert(0.0) += coefficient;
                }
                Amount::Resources(terms)
            }
            _ => {
                return Err(ValueError::new(
                    "a number and a resource bundle cannot be added",
                ));
            }
        };
        Ok(Self {
            amount: amount.canonicalize(),
            ..self.clone()
        })
    }

    pub fn sub(&self, other: &Self) -> Result<Self> {
        self.add(&other.negate())
    }

    pub fn negate(&self) -> Self {
        Self {
            amount: self.amount.negate(),
            ..self.clone()
        }
    }

    pub fn absolute(&self) -> Self {
        let amount = match &self.amount {
            Amount::Number(value) => Amount::Number(value.abs()),
            Amount::Resources(terms) => Amount::Resources(
                terms
                    .iter()
                    .map(|(resource, coefficient)| (*resource, coefficient.abs()))
                    .collect(),
            ),
        };
        Self {
            amount: amount.canonicalize(),
            ..self.clone()
        }
    }

    /// Multiplies amounts, rejecting the two unrepresentable cases (§5.3).
    fn multiply_amounts(a: &Amount, b: &Amount) -> Result<Amount> {
        Ok(match (a, b) {
            (Amount::Number(x), Amount::Number(y)) => Amount::Number(x * y),
            (Amount::Number(x), bundle) | (bundle, Amount::Number(x)) => bundle.scale(*x),
            (Amount::Resources(_), Amount::Resources(_)) => {
                return Err(ValueError::new(
                    "a product of two resource bundles is not representable",
                ));
            }
        })
    }

    pub fn mul(&self, other: &Self) -> Result<Self> {
        let second = self
            .second
            .checked_add(other.second)
            .ok_or_else(|| ValueError::new("exponent overflow"))?;
        let joule = self
            .joule
            .checked_add(other.joule)
            .ok_or_else(|| ValueError::new("exponent overflow"))?;
        let craft = self
            .craft
            .checked_add(other.craft)
            .ok_or_else(|| ValueError::new("exponent overflow"))?;

        let (amount, per) = match (self.per, other.per) {
            (Some(_), Some(_)) => {
                return Err(ValueError::new(
                    "a quantity cannot have two resources in the denominator",
                ));
            }
            (Some(per), None) => {
                if let Some((resource, coefficient)) = other.amount.single_term() {
                    if resource == per {
                        // The denominator cancels against the bundle (§5.3).
                        (self.amount.scale(coefficient), None)
                    } else {
                        (
                            Self::multiply_amounts(&self.amount, &other.amount)?,
                            Some(per),
                        )
                    }
                } else {
                    (
                        Self::multiply_amounts(&self.amount, &other.amount)?,
                        Some(per),
                    )
                }
            }
            (None, Some(per)) => {
                if let Some((resource, coefficient)) = self.amount.single_term() {
                    if resource == per {
                        (other.amount.scale(coefficient), None)
                    } else {
                        (
                            Self::multiply_amounts(&self.amount, &other.amount)?,
                            Some(per),
                        )
                    }
                } else {
                    (
                        Self::multiply_amounts(&self.amount, &other.amount)?,
                        Some(per),
                    )
                }
            }
            (None, None) => (Self::multiply_amounts(&self.amount, &other.amount)?, None),
        };

        Ok(Self {
            amount: amount.canonicalize(),
            per,
            second,
            joule,
            craft,
        })
    }

    pub fn reciprocal(&self) -> Result<Self> {
        let (amount, per) = match (&self.amount, self.per) {
            (Amount::Number(value), None) => (Amount::Number(1.0 / value), None),
            (Amount::Number(value), Some(below)) => (
                Amount::Resources(BTreeMap::from([(below, 1.0 / value)])),
                None,
            ),
            (Amount::Resources(_), below) => {
                let Some((resource, coefficient)) = self.amount.single_term() else {
                    return Err(ValueError::new(
                        "cannot divide by a quantity with several resources",
                    ));
                };
                match below {
                    None => (Amount::Number(1.0 / coefficient), Some(resource)),
                    Some(below) => (
                        Amount::Resources(BTreeMap::from([(below, 1.0 / coefficient)])),
                        Some(resource),
                    ),
                }
            }
        };
        let negate = |exponent: i32| {
            exponent
                .checked_neg()
                .ok_or_else(|| ValueError::new("exponent overflow"))
        };
        Ok(Self {
            amount: amount.canonicalize(),
            per,
            second: negate(self.second)?,
            joule: negate(self.joule)?,
            craft: negate(self.craft)?,
        })
    }

    pub fn div(&self, other: &Self) -> Result<Self> {
        self.mul(&other.reciprocal()?)
    }

    pub fn pow(&self, exponent: &Self) -> Result<Self> {
        let Some(power) = exponent.is_dimensionless_number() else {
            return Err(ValueError::new(
                "the exponent must be a dimensionless number",
            ));
        };
        let Some(base) = self.amount.number_value() else {
            return Err(ValueError::new(
                "a resource bundle cannot be raised to a power",
            ));
        };

        let has_units =
            self.per.is_some() || self.second != 0 || self.joule != 0 || self.craft != 0;
        if !has_units {
            return Ok(Self::number(base.powf(power)));
        }

        let power = as_integer(power)
            .ok_or_else(|| ValueError::new("a quantity with units needs an integer exponent"))?;
        if power == 0 {
            return Ok(Self::number(1.0));
        }
        if self.per.is_some() && power != 1 {
            return Err(ValueError::new(
                "a denominator resource is only representable with exponent one",
            ));
        }
        Ok(Self {
            amount: Amount::Number(base.powi(power)),
            per: self.per,
            second: self.second * power,
            joule: self.joule * power,
            craft: self.craft * power,
        })
    }

    /// Applies a quality comparison to every item term (§5.3).
    pub fn filter_quality(&self, comparison: Comparison, quality: Quality) -> Self {
        let keep = |resource: Resource| match resource.quality() {
            Some(actual) => comparison.eval(actual, quality),
            // A fluid has no quality, so it never satisfies a comparison.
            None => false,
        };
        let amount = match &self.amount {
            Amount::Number(_) => Amount::Number(0.0),
            Amount::Resources(terms) => Amount::Resources(
                terms
                    .iter()
                    .filter(|(resource, _)| keep(**resource))
                    .map(|(resource, coefficient)| (*resource, *coefficient))
                    .collect(),
            ),
        };
        let per = match self.per {
            Some(resource) if keep(resource) => Some(resource),
            Some(_) => {
                return Self {
                    amount: Amount::Number(0.0),
                    per: None,
                    ..self.clone()
                };
            }
            None => None,
        };
        Self {
            amount: amount.canonicalize(),
            per,
            ..self.clone()
        }
    }

    /// Keeps the terms whose coefficient satisfies the comparison (§5.3).
    ///
    /// `q > 0` is the positive terms and `q < 0` the negative ones, which is how
    /// a bundle is split by sign.
    pub fn filter_amount(&self, comparison: Comparison, threshold: f64) -> Self {
        let amount = match &self.amount {
            Amount::Number(value) => {
                if comparison.eval_f64(*value, threshold) {
                    Amount::Number(*value)
                } else {
                    Amount::Number(0.0)
                }
            }
            Amount::Resources(terms) => Amount::Resources(
                terms
                    .iter()
                    .filter(|(_, coefficient)| comparison.eval_f64(**coefficient, threshold))
                    .map(|(resource, coefficient)| (*resource, *coefficient))
                    .collect(),
            ),
        };
        Self {
            amount: amount.canonicalize(),
            ..self.clone()
        }
    }

    /// Keeps the terms a reference selector matches (§5.3).
    pub fn select(&self, selector: &ResourceRef) -> Self {
        let matches = |resource: Resource| {
            resource.id() == selector.id
                && selector
                    .quality
                    .is_none_or(|quality| resource.quality() == Some(quality))
        };
        let amount = match &self.amount {
            Amount::Number(_) => Amount::Number(0.0),
            Amount::Resources(terms) => Amount::Resources(
                terms
                    .iter()
                    .filter(|(resource, _)| matches(**resource))
                    .map(|(resource, coefficient)| (*resource, *coefficient))
                    .collect(),
            ),
        };
        let per = match self.per {
            Some(resource) if matches(resource) => Some(resource),
            Some(_) => {
                return Self {
                    amount: Amount::Number(0.0),
                    per: None,
                    ..self.clone()
                };
            }
            None => None,
        };
        Self {
            amount: amount.canonicalize(),
            per,
            ..self.clone()
        }
    }

    /// Keeps only terms of one medium (§5.3).
    pub fn select_medium(&self, item: bool) -> Self {
        let amount = match &self.amount {
            Amount::Number(_) => Amount::Number(0.0),
            Amount::Resources(terms) => Amount::Resources(
                terms
                    .iter()
                    .filter(|(resource, _)| matches!(resource, Resource::Item(_)) == item)
                    .map(|(resource, coefficient)| (*resource, *coefficient))
                    .collect(),
            ),
        };
        let per = match self.per {
            Some(resource) if matches!(resource, Resource::Item(_)) == item => Some(resource),
            _ => None,
        };
        Self {
            amount: amount.canonicalize(),
            per,
            ..self.clone()
        }
    }
}

fn as_integer(value: f64) -> Option<i32> {
    if !value.is_finite() || (value - value.round()).abs() > APPROX_ZERO {
        return None;
    }
    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return None;
    }
    Some(rounded as i32)
}

/// A quality comparison, which the filter operators use (§5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    NotEqual,
}

impl Comparison {
    pub fn eval<T: Ord>(self, left: T, right: T) -> bool {
        match self {
            Comparison::Less => left < right,
            Comparison::Greater => left > right,
            Comparison::LessEqual => left <= right,
            Comparison::GreaterEqual => left >= right,
            Comparison::Equal => left == right,
            Comparison::NotEqual => left != right,
        }
    }

    /// The same comparison on plain numbers.
    pub fn eval_f64(self, left: f64, right: f64) -> bool {
        match self {
            Comparison::Less => left < right,
            Comparison::Greater => left > right,
            Comparison::LessEqual => left <= right,
            Comparison::GreaterEqual => left >= right,
            Comparison::Equal => left == right,
            Comparison::NotEqual => left != right,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Comparison::Less => "<",
            Comparison::Greater => ">",
            Comparison::LessEqual => "<=",
            Comparison::GreaterEqual => ">=",
            Comparison::Equal => "==",
            Comparison::NotEqual => "!=",
        }
    }
}

/// The `resource` reference facet: identity, with unspecified quality (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceRef {
    pub id: EntryId,
    pub is_fluid: bool,
    pub quality: Option<Quality>,
}

/// The `recipe` reference facet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeRef {
    pub id: RecipeId,
    pub quality: Option<Quality>,
}

/// The cap a stack rate or capacity is measured against (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StackLimit {
    /// The item's own stack size.
    Full,
    /// At most this many items per stack.
    Limit(u32),
}

impl Ord for StackLimit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (StackLimit::Full, StackLimit::Full) => std::cmp::Ordering::Equal,
            (StackLimit::Full, StackLimit::Limit(_)) => std::cmp::Ordering::Less,
            (StackLimit::Limit(_), StackLimit::Full) => std::cmp::Ordering::Greater,
            // Limits order descending because that is how they print (§7.6).
            (StackLimit::Limit(left), StackLimit::Limit(right)) => right.cmp(left),
        }
    }
}

impl PartialOrd for StackLimit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Item stacks per limit, or a fluid volume (§3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Stacks {
    Items(BTreeMap<StackLimit, f64>),
    Fluid(f64),
}

impl Stacks {
    pub fn items(limit: StackLimit, amount: f64) -> Self {
        Stacks::Items(BTreeMap::from([(limit, amount)])).canonicalize()
    }

    pub fn canonicalize(self) -> Self {
        match self {
            Stacks::Items(mut terms) => {
                terms.retain(|_, amount| !is_zero(*amount));
                Stacks::Items(terms)
            }
            fluid => fluid,
        }
    }

    /// Combines two of the same medium, which is the only case `+` accepts.
    pub fn add(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Stacks::Items(left), Stacks::Items(right)) => {
                let mut terms = left.clone();
                for (limit, amount) in right {
                    *terms.entry(*limit).or_insert(0.0) += amount;
                }
                Some(Stacks::Items(terms).canonicalize())
            }
            (Stacks::Fluid(left), Stacks::Fluid(right)) => Some(Stacks::Fluid(left + right)),
            _ => None,
        }
    }

    pub fn scale(&self, factor: f64) -> Self {
        match self {
            Stacks::Items(terms) => Stacks::Items(
                terms
                    .iter()
                    .map(|(limit, amount)| (*limit, amount * factor))
                    .collect(),
            )
            .canonicalize(),
            Stacks::Fluid(amount) => Stacks::Fluid(amount * factor),
        }
    }

    pub fn negate(&self) -> Self {
        self.scale(-1.0)
    }

    pub fn absolute(&self) -> Self {
        match self {
            Stacks::Items(terms) => Stacks::Items(
                terms
                    .iter()
                    .map(|(limit, amount)| (*limit, amount.abs()))
                    .collect(),
            )
            .canonicalize(),
            Stacks::Fluid(amount) => Stacks::Fluid(amount.abs()),
        }
    }

    /// The amount when every term uses `limit`, which `:stacked-belt:` needs.
    pub fn only_at(&self, limit: StackLimit) -> Option<f64> {
        match self {
            Stacks::Items(terms) if terms.len() == 1 => terms.get(&limit).copied(),
            _ => None,
        }
    }

    /// Applies these stacks to a resource (§5.2 rules 1 and 2). A medium
    /// mismatch is `None`; the caller turns that into an error.
    pub fn apply(&self, resource: Resource, coefficient: f64, stack_size: u32) -> Option<f64> {
        match (self, resource) {
            (Stacks::Items(terms), Resource::Item(_)) => Some(
                coefficient
                    * terms
                        .iter()
                        .map(|(limit, amount)| {
                            let cap = match limit {
                                StackLimit::Full => stack_size,
                                StackLimit::Limit(limit) => (*limit).min(stack_size),
                            };
                            amount * f64::from(cap)
                        })
                        .sum::<f64>(),
            ),
            (Stacks::Fluid(amount), Resource::Fluid(_)) => Some(coefficient * amount),
            _ => None,
        }
    }
}

impl Quantity {
    /// A pure time amount, which is what turns a rate into a capacity.
    pub fn pure_time(&self) -> Option<f64> {
        if self.per.is_some() || self.second != 1 || self.joule != 0 || self.craft != 0 {
            return None;
        }
        self.amount.number_value()
    }

    /// A pure amount of one resource, which is what a rate applies to.
    pub fn pure_resource_amount(&self) -> Option<(Resource, f64)> {
        if self.per.is_some() || self.second != 0 || self.joule != 0 || self.craft != 0 {
            return None;
        }
        self.amount.single_term()
    }
}

/// A module list: modules with their qualities, each with a count (§6.2).
pub type Modules = BTreeMap<(EntryId, Quality), u32>;

/// One beacon type holding one module list (§6.2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BeaconGroup {
    pub beacon: EntryId,
    pub quality: Quality,
    pub modules: Modules,
}

/// Modules in a machine plus its beacon groups (§6.2).
#[derive(Debug, Clone, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct Config {
    pub modules: Modules,
    pub beacons: BTreeMap<BeaconGroup, u32>,
}

impl Config {
    pub fn module(module: EntryId, quality: Quality) -> Self {
        Self {
            modules: Modules::from([((module, quality), 1)]),
            ..Self::default()
        }
    }

    pub fn beacon(beacon: EntryId, quality: Quality) -> Self {
        let group = BeaconGroup {
            beacon,
            quality,
            modules: Modules::new(),
        };
        Self {
            beacons: BTreeMap::from([(group, 1)]),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty() && self.beacons.is_empty()
    }

    pub fn total_beacons(&self) -> u32 {
        self.beacons.values().sum()
    }

    /// `+` merges module counts and beacon-group counts (§5.5).
    pub fn add(&self, other: &Self) -> Self {
        let mut modules = self.modules.clone();
        for (module, count) in &other.modules {
            *modules.entry(*module).or_insert(0) += count;
        }
        let mut beacons = self.beacons.clone();
        for (group, count) in &other.beacons {
            *beacons.entry(group.clone()).or_insert(0) += count;
        }
        Self { modules, beacons }
    }

    /// Scales counts, which requires a nonnegative integer factor (§5.5).
    pub fn scale(&self, factor: f64) -> Option<Self> {
        let factor = as_count(factor)?;
        let modules = self
            .modules
            .iter()
            .map(|(module, count)| (*module, count * factor))
            .collect();
        let beacons = self
            .beacons
            .iter()
            .map(|(group, count)| (group.clone(), count * factor))
            .collect();
        Some(Self { modules, beacons })
    }

    /// A module list merged into another module list (§5.2).
    pub fn merge_modules(&self, other: &Self) -> Option<Self> {
        if !self.beacons.is_empty() || !other.beacons.is_empty() {
            return None;
        }
        Some(Self {
            modules: Self::add(self, other).modules,
            beacons: BTreeMap::new(),
        })
    }

    /// Modules attached to a config that is one beacon and nothing else (§5.2).
    ///
    /// Juxtaposition may only build a module list or fill an empty beacon.
    /// Everything else is a general config combination and needs `+`.
    pub fn attach_modules(&self, modules: &Self) -> Option<Self> {
        if !self.modules.is_empty() || !modules.beacons.is_empty() || modules.modules.is_empty() {
            return None;
        }
        if self.beacons.len() != 1 {
            return None;
        }
        let (group, count) = self.beacons.iter().next()?;
        if !group.modules.is_empty() {
            return None;
        }
        let mut merged = group.modules.clone();
        for (module, module_count) in &modules.modules {
            *merged.entry(*module).or_insert(0) += module_count;
        }
        let group = BeaconGroup {
            modules: merged,
            ..group.clone()
        };
        Some(Self {
            modules: Modules::new(),
            beacons: BTreeMap::from([(group, *count)]),
        })
    }

    /// Rewrites the quality of every module and beacon the single-identity
    /// rule allows (§3.3).
    pub fn qualify(&self, quality: Quality) -> Option<Self> {
        if self.beacons.len() == 1 && self.modules.is_empty() {
            let (group, count) = self.beacons.iter().next()?;
            let group = BeaconGroup {
                quality,
                ..group.clone()
            };
            return Some(Self {
                modules: Modules::new(),
                beacons: BTreeMap::from([(group, *count)]),
            });
        }
        let sole = self.modules.len() == 1 && self.beacons.is_empty();
        let sole = sole && self.modules.values().next().copied() == Some(1);
        if !sole {
            return None;
        }
        let (module, _) = self.modules.iter().next()?;
        Some(Self {
            modules: Modules::from([((module.0, quality), 1)]),
            beacons: BTreeMap::new(),
        })
    }
}

/// A machine prototype with a config and an optional count (§6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub entry: EntryId,
    pub quality: Quality,
    pub config: Config,
    pub count: Option<f64>,
}

impl Machine {
    pub fn new(entry: EntryId, quality: Quality) -> Self {
        Self {
            entry,
            quality,
            config: Config::default(),
            count: None,
        }
    }

    pub fn count(&self) -> f64 {
        self.count.unwrap_or(1.0)
    }

    /// The machine with a config merged in (§5.2).
    pub fn with_config(&self, config: &Config) -> Self {
        Self {
            config: self.config.add(config),
            ..self.clone()
        }
    }
}

impl Machine {
    /// Machines add when they are the same prototype, quality, and config.
    fn same(left: &Self, right: &Self) -> Option<Self> {
        if left.entry != right.entry || left.quality != right.quality || left.config != right.config
        {
            return None;
        }
        Some(Self {
            count: Some(left.count() + right.count()),
            ..left.clone()
        })
    }
}

/// A nonnegative integer count, or `None`.
fn as_count(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || (value - value.round()).abs() > APPROX_ZERO {
        return None;
    }
    let rounded = value.round();
    if rounded > f64::from(u32::MAX) {
        return None;
    }
    Some(rounded as u32)
}

/// How a process is sized (§6.4): the rows carry the magnitude, this is the
/// kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// One machine per row, not yet sized.
    Unscaled,
    /// Row scales are machine counts.
    Machines,
    /// Row scales are machine-seconds.
    MachineSeconds,
    /// Nothing was produced: an empty process whose unit is not known (§5.4).
    Unspecified,
}

/// One process row: a machine with its config, making one recipe at one
/// quality (§6.4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowKey {
    pub machine: EntryId,
    pub machine_quality: Quality,
    pub config: Config,
    pub recipe: RecipeId,
    pub recipe_quality: Quality,
}

/// A solved production result (§6.4). Never coexists with facets.
#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub extent: Extent,
    /// Row scale in the extent's unit.
    pub rows: BTreeMap<RowKey, f64>,
    /// Net rows written by property assignment, in the extent's unit (§2.2).
    pub net: BTreeMap<Resource, f64>,
}

impl Process {
    pub fn new(extent: Extent, rows: BTreeMap<RowKey, f64>) -> Self {
        Self {
            extent,
            rows,
            net: BTreeMap::new(),
        }
    }

    pub fn scale(&self, factor: f64) -> Self {
        Self {
            extent: self.extent,
            rows: self
                .rows
                .iter()
                .map(|(key, scale)| (key.clone(), scale * factor))
                .collect(),
            net: self
                .net
                .iter()
                .map(|(resource, amount)| (*resource, amount * factor))
                .collect(),
        }
    }

    pub fn negate(&self) -> Self {
        self.scale(-1.0)
    }

    /// Adds rows when both processes are sized by the same unit (§5.4).
    pub fn add(&self, other: &Self) -> Option<Self> {
        // An empty process is the identity, whichever unit it was built from.
        if self.extent == Extent::Unspecified {
            return Some(other.clone());
        }
        if other.extent == Extent::Unspecified {
            return Some(self.clone());
        }
        if self.extent != other.extent {
            return None;
        }
        let mut rows = self.rows.clone();
        for (key, scale) in &other.rows {
            *rows.entry(key.clone()).or_insert(0.0) += scale;
        }
        rows.retain(|_, scale| !is_zero(*scale));
        let mut net = self.net.clone();
        for (resource, amount) in &other.net {
            *net.entry(*resource).or_insert(0.0) += amount;
        }
        net.retain(|_, amount| !is_zero(*amount));
        Some(Self {
            extent: self.extent,
            rows,
            net,
        })
    }

    /// One recipe from a process whose rows are the quality variants of it
    /// (§5.4): the only row, else the only common one, else nothing.
    pub fn collapsed(&self) -> Option<Self> {
        if self.rows.len() == 1 {
            return Some(self.clone());
        }
        let first = self.rows.keys().next()?;
        let one_recipe = self.rows.keys().all(|key| {
            key.machine == first.machine
                && key.machine_quality == first.machine_quality
                && key.config == first.config
                && key.recipe == first.recipe
        });
        if !one_recipe {
            return None;
        }
        let common: Vec<(&RowKey, &f64)> = self
            .rows
            .iter()
            .filter(|(key, _)| key.recipe_quality == Quality::Normal)
            .collect();
        let (key, scale) = common.first().copied().filter(|_| common.len() == 1)?;
        Some(Self {
            extent: self.extent,
            rows: BTreeMap::from([(key.clone(), *scale)]),
            net: self.net.clone(),
        })
    }

    /// Scaling by a number sizes the process: one recipe, counted in machines.
    pub fn scaled(&self, factor: f64) -> Option<Self> {
        let base = if self.extent == Extent::Unscaled {
            self.collapsed()?
        } else {
            self.clone()
        };
        Some(Self {
            extent: if self.extent == Extent::Unscaled {
                Extent::Machines
            } else {
                self.extent
            },
            rows: base
                .rows
                .iter()
                .map(|(key, scale)| (key.clone(), scale * factor))
                .collect(),
            net: base
                .net
                .iter()
                .map(|(resource, amount)| (*resource, amount * factor))
                .collect(),
        })
    }

    /// `Machines · seconds` gives machine-seconds (§5.4).
    pub fn times_seconds(&self, seconds: f64) -> Option<Self> {
        match self.extent {
            Extent::Machines => Some(Self {
                extent: Extent::MachineSeconds,
                rows: self
                    .rows
                    .iter()
                    .map(|(key, scale)| (key.clone(), scale * seconds))
                    .collect(),
                net: self
                    .net
                    .iter()
                    .map(|(resource, amount)| (*resource, amount * seconds))
                    .collect(),
            }),
            Extent::Unscaled => Some(Self {
                extent: Extent::MachineSeconds,
                // An unscaled row is one machine, so its machine-seconds are
                // the seconds themselves.
                rows: self.rows.keys().map(|key| (key.clone(), seconds)).collect(),
                net: self
                    .net
                    .iter()
                    .map(|(resource, amount)| (*resource, amount * seconds))
                    .collect(),
            }),
            Extent::MachineSeconds | Extent::Unspecified => None,
        }
    }

    /// `MachineSeconds ÷ seconds` gives machines (§5.4).
    pub fn divided_by_seconds(&self, seconds: f64) -> Option<Self> {
        match self.extent {
            Extent::MachineSeconds => Some(Self {
                extent: Extent::Machines,
                rows: self
                    .rows
                    .iter()
                    .map(|(key, scale)| (key.clone(), scale / seconds))
                    .collect(),
                net: self
                    .net
                    .iter()
                    .map(|(resource, amount)| (*resource, amount / seconds))
                    .collect(),
            }),
            _ => None,
        }
    }

    /// The machine total, treating recipe qualities of one row as alternatives.
    pub fn machines(&self) -> f64 {
        crate::new_eval::process::machines(&self.rows)
    }
}

/// The facets of one value (§3.1). The MVP implements five of the eight.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MultiValue {
    pub quantity: Option<Quantity>,
    pub stack_rate: Option<Stacks>,
    pub stack_capacity: Option<Stacks>,
    pub machine: Option<Machine>,
    pub config: Option<Config>,
    pub resource: Option<ResourceRef>,
    pub recipe: Option<RecipeRef>,
}

impl MultiValue {
    pub fn quantity(quantity: Quantity) -> Self {
        Self {
            quantity: Some(quantity),
            ..Self::default()
        }
    }

    pub fn facets(&self) -> String {
        let mut names = Vec::new();
        if self.quantity.is_some() {
            names.push("quantity");
        }
        if self.stack_rate.is_some() {
            names.push("stack rate");
        }
        if self.stack_capacity.is_some() {
            names.push("stack capacity");
        }
        if self.machine.is_some() {
            names.push("machine");
        }
        if self.config.is_some() {
            names.push("config");
        }
        if self.resource.is_some() {
            names.push("resource");
        }
        if self.recipe.is_some() {
            names.push("recipe");
        }
        if names.is_empty() {
            "none".to_string()
        } else {
            names.join(" ")
        }
    }

    fn is_empty(&self) -> bool {
        self.quantity.is_none()
            && self.stack_rate.is_none()
            && self.stack_capacity.is_none()
            && self.machine.is_none()
            && self.config.is_none()
            && self.resource.is_none()
            && self.recipe.is_none()
    }

    /// Drops every facet that did not combine, and reports total loss (P3).
    fn or_dropped(self, operator: &str, left: &Self, right: &Self) -> Result<Self> {
        if self.is_empty() {
            return Err(ValueError::new(format!(
                "`{operator}` dropped every facet (left: {}, right: {})",
                left.facets(),
                right.facets()
            )));
        }
        Ok(self)
    }

    fn combine<T>(
        left: &Option<T>,
        right: &Option<T>,
        op: impl Fn(&T, &T) -> Option<T>,
    ) -> Option<T> {
        match (left.as_ref(), right.as_ref()) {
            (Some(left), Some(right)) => op(left, right),
            _ => None,
        }
    }

    pub fn add(&self, other: &Self) -> Result<Self> {
        let result = Self {
            quantity: Self::combine(&self.quantity, &other.quantity, |a, b| a.add(b).ok()),
            stack_rate: Self::combine(&self.stack_rate, &other.stack_rate, Stacks::add),
            stack_capacity: Self::combine(&self.stack_capacity, &other.stack_capacity, Stacks::add),
            machine: Self::combine(&self.machine, &other.machine, Machine::same),
            config: Self::combine(&self.config, &other.config, |a, b| Some(a.add(b))),
            ..Self::default()
        };
        result.or_dropped("+", self, other)
    }

    pub fn sub(&self, other: &Self) -> Result<Self> {
        let negated = other.negate()?;
        self.add(&negated).map_err(|_| {
            ValueError::new(format!(
                "`-` dropped every facet (left: {}, right: {})",
                self.facets(),
                other.facets()
            ))
        })
    }

    pub fn mul(&self, other: &Self) -> Result<Self> {
        let quantity = Self::combine(&self.quantity, &other.quantity, |a, b| a.mul(b).ok());
        let stack_capacity = self
            .stack_rate
            .as_ref()
            .zip(other.quantity.as_ref().and_then(Quantity::pure_time))
            .map(|(stacks, seconds)| stacks.scale(seconds))
            .or_else(|| {
                other
                    .stack_rate
                    .as_ref()
                    .zip(self.quantity.as_ref().and_then(Quantity::pure_time))
                    .map(|(stacks, seconds)| stacks.scale(seconds))
            });
        Self {
            quantity,
            stack_capacity,
            ..Self::default()
        }
        .or_dropped("*", self, other)
    }

    pub fn div(&self, other: &Self) -> Result<Self> {
        let quantity = Self::combine(&self.quantity, &other.quantity, |a, b| a.div(b).ok());
        let stack_rate = self
            .stack_capacity
            .as_ref()
            .zip(other.quantity.as_ref().and_then(Quantity::pure_time))
            .map(|(stacks, seconds)| stacks.scale(1.0 / seconds));
        Self {
            quantity,
            stack_rate,
            ..Self::default()
        }
        .or_dropped("/", self, other)
    }

    pub fn pow(&self, other: &Self) -> Result<Self> {
        let quantity = Self::combine(&self.quantity, &other.quantity, |a, b| a.pow(b).ok());
        Self {
            quantity,
            ..Self::default()
        }
        .or_dropped("^", self, other)
    }

    pub fn negate(&self) -> Result<Self> {
        let result = Self {
            quantity: self.quantity.as_ref().map(Quantity::negate),
            stack_rate: self.stack_rate.as_ref().map(Stacks::negate),
            stack_capacity: self.stack_capacity.as_ref().map(Stacks::negate),
            machine: self.machine.as_ref().map(|machine| Machine {
                count: Some(-machine.count()),
                ..machine.clone()
            }),
            config: None,
            resource: self.resource,
            recipe: self.recipe,
        };
        if result.is_empty() {
            return Err(ValueError::new("unary `-` dropped every facet"));
        }
        Ok(result)
    }

    pub fn absolute(&self) -> Result<Self> {
        let result = Self {
            quantity: self.quantity.as_ref().map(Quantity::absolute),
            stack_rate: self.stack_rate.as_ref().map(Stacks::absolute),
            stack_capacity: self.stack_capacity.as_ref().map(Stacks::absolute),
            machine: self.machine.clone(),
            config: self.config.clone(),
            resource: self.resource,
            recipe: self.recipe,
        };
        if result.is_empty() {
            return Err(ValueError::new("`.abs` dropped every facet"));
        }
        Ok(result)
    }

    /// Scales by a dimensionless factor, dropping the reference facets (§5.1).
    pub fn scale(&self, factor: f64) -> Result<Self> {
        let result = Self {
            quantity: self.quantity.as_ref().map(|quantity| Quantity {
                amount: quantity.amount.scale(factor),
                ..quantity.clone()
            }),
            stack_rate: self.stack_rate.as_ref().map(|stacks| stacks.scale(factor)),
            stack_capacity: self
                .stack_capacity
                .as_ref()
                .map(|stacks| stacks.scale(factor)),
            machine: self.machine.as_ref().map(|machine| Machine {
                count: Some(machine.count() * factor),
                ..machine.clone()
            }),
            config: self.config.as_ref().and_then(|config| config.scale(factor)),
            ..Self::default()
        };
        result.or_dropped("scaling", self, self)
    }

    /// Applies the quality operand of §3.3.
    pub fn qualify(&self, quality: Quality) -> Result<Self> {
        let quantity = self.quantity.as_ref().map(|quantity| Quantity {
            amount: quantity
                .amount
                .map_resources(|resource| resource.with_quality(quality)),
            per: quantity.per.map(|resource| resource.with_quality(quality)),
            ..quantity.clone()
        });
        let resource = self.resource.map(|resource| ResourceRef {
            quality: if resource.is_fluid {
                resource.quality
            } else {
                Some(quality)
            },
            ..resource
        });
        let recipe = self.recipe.map(|recipe| RecipeRef {
            quality: Some(quality),
            ..recipe
        });
        let machine = self.machine.as_ref().map(|machine| Machine {
            quality,
            ..machine.clone()
        });
        let config = match &self.config {
            Some(config) => match config.qualify(quality) {
                Some(config) => Some(config),
                None => {
                    return Err(ValueError::new(
                        "a quality operand cannot pick one module or beacon out of that config",
                    ));
                }
            },
            None => None,
        };
        let qualified = Self {
            quantity,
            stack_rate: self.stack_rate.clone(),
            stack_capacity: self.stack_capacity.clone(),
            machine,
            config,
            resource,
            recipe,
        };
        if qualified.is_empty() {
            return Err(ValueError::new("a quality operand dropped every facet"));
        }
        Ok(qualified)
    }

    pub fn filter_amount(&self, comparison: Comparison, threshold: f64) -> Result<Self> {
        let Some(quantity) = &self.quantity else {
            return Err(ValueError::new(format!(
                "`{}` dropped every facet (left: {}, right: quantity)",
                comparison.symbol(),
                self.facets()
            )));
        };
        Ok(Self::quantity(
            quantity.filter_amount(comparison, threshold),
        ))
    }

    pub fn filter_quality(&self, comparison: Comparison, quality: Quality) -> Result<Self> {
        let Some(quantity) = &self.quantity else {
            return Err(ValueError::new(format!(
                "`{}` dropped every facet (left: {}, right: quality)",
                comparison.symbol(),
                self.facets()
            )));
        };
        Ok(Self::quantity(quantity.filter_quality(comparison, quality)))
    }

    pub fn select(&self, selector: &ResourceRef) -> Result<Self> {
        let Some(quantity) = &self.quantity else {
            return Err(ValueError::new(
                "`.` selection needs a quantity to select from",
            ));
        };
        Ok(Self::quantity(quantity.select(selector)))
    }

    pub fn select_medium(&self, item: bool) -> Result<Self> {
        let Some(quantity) = &self.quantity else {
            return Err(ValueError::new(
                "`.` selection needs a quantity to select from",
            ));
        };
        Ok(Self::quantity(quantity.select_medium(item)))
    }

    /// Applies a rate or capacity to a pure item or fluid amount (§5.2).
    fn apply_stacks(
        &self,
        other: &Self,
        stack_size: &dyn Fn(Resource) -> Option<u32>,
    ) -> Result<Option<Self>> {
        let (stacks, is_rate) = match (&self.stack_rate, &self.stack_capacity) {
            (Some(rate), _) => (rate, true),
            (None, Some(capacity)) => (capacity, false),
            (None, None) => return Ok(None),
        };
        let Some(quantity) = &other.quantity else {
            return Ok(None);
        };
        let Some((resource, coefficient)) = quantity.pure_resource_amount() else {
            return Ok(None);
        };
        let size = match resource {
            Resource::Item(_) => match stack_size(resource) {
                Some(size) => size,
                None => return Ok(None),
            },
            Resource::Fluid(_) => 0,
        };
        let Some(amount) = stacks.apply(resource, coefficient, size) else {
            return Err(ValueError::new(
                "a stack rate cannot be applied to a resource of the other medium",
            ));
        };
        Ok(Some(Self {
            quantity: Some(Quantity {
                amount: Amount::Resources(BTreeMap::from([(resource, amount)])).canonicalize(),
                per: None,
                second: if is_rate { -1 } else { 0 },
                joule: 0,
                craft: 0,
            }),
            ..Self::default()
        }))
    }

    /// Collapses every stack facet from cap one onto the stacking level (§6.1).
    pub fn stacked(&self, level: u32) -> Result<Self> {
        let level = StackLimit::Limit(level.clamp(1, 4));
        let collapse = |stacks: &Stacks| -> Result<Stacks> {
            let Some(amount) = stacks.only_at(StackLimit::Limit(1)) else {
                return Err(ValueError::new(
                    "`:stacked-belt:` needs a rate whose only stack limit is one",
                ));
            };
            Ok(Stacks::items(level, amount))
        };
        let stack_rate = self.stack_rate.as_ref().map(collapse).transpose()?;
        let stack_capacity = self.stack_capacity.as_ref().map(collapse).transpose()?;
        if stack_rate.is_none() && stack_capacity.is_none() {
            return Err(ValueError::new(format!(
                "`:stacked-belt:` needs a stack rate or capacity ({})",
                self.facets()
            )));
        }
        Ok(Self {
            stack_rate,
            stack_capacity,
            ..Self::default()
        })
    }
}

/// A value: facets, or a quality operand (§3.4).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Multi(MultiValue),
    Process(Process),
    Quality(Quality),
}

impl Value {
    pub fn multi(value: MultiValue) -> Self {
        Value::Multi(value)
    }

    pub fn quantity(quantity: Quantity) -> Self {
        Value::Multi(MultiValue::quantity(quantity))
    }

    pub fn number(value: f64) -> Self {
        Self::quantity(Quantity::number(value))
    }

    /// The dimensionless number this value is, if it is one.
    pub fn dimensionless(&self) -> Option<f64> {
        match self {
            Value::Multi(multi) => multi.quantity.as_ref()?.is_dimensionless_number(),
            Value::Process(_) | Value::Quality(_) => None,
        }
    }

    fn as_multi(&self, operator: &str) -> Result<&MultiValue> {
        match self {
            Value::Multi(multi) => Ok(multi),
            Value::Process(_) => Err(ValueError::new(format!(
                "`{operator}` needs facets, not a process"
            ))),
            Value::Quality(_) => Err(ValueError::new(format!(
                "`{operator}` needs a value, not a quality"
            ))),
        }
    }

    pub fn as_process(&self) -> Option<&Process> {
        match self {
            Value::Process(process) => Some(process),
            _ => None,
        }
    }

    pub fn add(&self, other: &Self) -> Result<Self> {
        if let (Some(left), Some(right)) = (self.as_process(), other.as_process()) {
            return left
                .add(right)
                .map(Value::Process)
                .ok_or_else(|| ValueError::new("processes add only when their extents agree"));
        }
        Ok(Value::Multi(self.as_multi("+")?.add(other.as_multi("+")?)?))
    }

    pub fn sub(&self, other: &Self) -> Result<Self> {
        if let Some(right) = other.as_process() {
            return self.add(&Value::Process(right.negate()));
        }
        Ok(Value::Multi(self.as_multi("-")?.sub(other.as_multi("-")?)?))
    }

    pub fn mul(&self, other: &Self) -> Result<Self> {
        if let Some(factor) = self.dimensionless() {
            return self.scale_value(other, factor);
        }
        if let Some(factor) = other.dimensionless() {
            return self
                .scale_value(self, factor)
                .and_then(|_| self.scale_value(self, factor));
        }
        // Machines multiplied by a time become machine-seconds (§5.4).
        if let (Some(process), Some(seconds)) = (self.as_process(), other.pure_time()) {
            return process
                .times_seconds(seconds)
                .map(Value::Process)
                .ok_or_else(|| {
                    ValueError::new("a machine-seconds process cannot be multiplied by a time")
                });
        }
        if let (Some(seconds), Some(process)) = (self.pure_time(), other.as_process()) {
            return process
                .times_seconds(seconds)
                .map(Value::Process)
                .ok_or_else(|| {
                    ValueError::new("a machine-seconds process cannot be multiplied by a time")
                });
        }
        Ok(Value::Multi(self.as_multi("*")?.mul(other.as_multi("*")?)?))
    }

    pub fn div(&self, other: &Self) -> Result<Self> {
        if let Some(factor) = other.dimensionless() {
            return self.scale_value(self, 1.0 / factor);
        }
        if let (Some(process), Some(seconds)) = (self.as_process(), other.pure_time()) {
            return process
                .divided_by_seconds(seconds)
                .map(Value::Process)
                .ok_or_else(|| {
                    ValueError::new("only a machine-seconds process can be divided by a time")
                });
        }
        Ok(Value::Multi(self.as_multi("/")?.div(other.as_multi("/")?)?))
    }

    fn scale_value(&self, target: &Self, factor: f64) -> Result<Self> {
        match target {
            Value::Process(process) => {
                process.scaled(factor).map(Value::Process).ok_or_else(|| {
                    ValueError::new(
                        "scaling a process needs one recipe, and that process has several",
                    )
                })
            }
            _ => Ok(Value::Multi(target.as_multi("*")?.scale(factor)?)),
        }
    }

    /// A pure time amount, which turns machines into machine-seconds.
    pub fn pure_time(&self) -> Option<f64> {
        match self {
            Value::Multi(multi) => multi.quantity.as_ref()?.pure_time(),
            _ => None,
        }
    }

    pub fn pow(&self, other: &Self) -> Result<Self> {
        Ok(Value::Multi(self.as_multi("^")?.pow(other.as_multi("^")?)?))
    }

    pub fn negate(&self) -> Result<Self> {
        match self {
            Value::Process(process) => Ok(Value::Process(process.negate())),
            _ => Ok(Value::Multi(self.as_multi("-")?.negate()?)),
        }
    }

    pub fn absolute(&self) -> Result<Self> {
        if self.as_process().is_some() {
            return Err(ValueError::new("`.abs` refuses a process"));
        }
        Ok(Value::Multi(self.as_multi(".abs")?.absolute()?))
    }

    /// Juxtaposition (§5.2). The chain has already been folded to the right by
    /// the evaluator, so this is one binary step.
    pub fn juxtapose(
        &self,
        other: &Self,
        stack_size: &dyn Fn(Resource) -> Option<u32>,
        validate: &dyn Fn(&MultiValue) -> Result<(), ValueError>,
    ) -> Result<Self> {
        if let Value::Quality(quality) = other {
            return Err(ValueError::new(format!(
                "a quality may only be the left operand of juxtaposition, found `:{}:` on the right",
                quality.name()
            )));
        }
        if let Value::Quality(quality) = self {
            return Ok(Value::Multi(
                other.as_multi("juxtaposition")?.qualify(*quality)?,
            ));
        }
        if let Some(factor) = self.dimensionless() {
            return self.scale_value(other, factor);
        }
        if let Some(factor) = other.dimensionless() {
            return self.scale_value(self, factor);
        }
        let (left, right) = (
            self.as_multi("juxtaposition")?,
            other.as_multi("juxtaposition")?,
        );
        if let Some(applied) = left.apply_stacks(right, stack_size)? {
            return Ok(Value::Multi(applied));
        }
        // Configs combine per facet: modules onto a beacon group, or two module
        // lists merged; a machine absorbs a config (§5.2).
        let config = match (&left.config, &right.config) {
            (Some(left), Some(right)) => left
                .attach_modules(right)
                .or_else(|| left.merge_modules(right)),
            _ => None,
        };
        let machine = match (&left.machine, &right.config) {
            (Some(machine), Some(config)) => Some(machine.with_config(config)),
            _ => None,
        };
        if config.is_some() || machine.is_some() {
            let produced = MultiValue {
                machine,
                config,
                ..MultiValue::default()
            };
            validate(&produced)?;
            return Ok(Value::Multi(produced));
        }
        Err(ValueError::new(format!(
            "juxtaposition matched no rule (left: {}, right: {})",
            left.facets(),
            right.facets()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hactorio_data::EntryId;

    fn iron() -> Resource {
        Resource::Item(Item {
            id: EntryId::from_index(1),
            quality: Quality::Normal,
        })
    }

    fn copper() -> Resource {
        Resource::Item(Item {
            id: EntryId::from_index(2),
            quality: Quality::Normal,
        })
    }

    #[test]
    fn bundles_add_term_wise_and_keep_negative_terms() {
        let sum = Quantity::resource(iron())
            .sub(&Quantity::resource(copper()))
            .unwrap();
        let Amount::Resources(terms) = &sum.amount else {
            panic!("expected a bundle");
        };
        assert_eq!(terms[&iron()], 1.0);
        assert_eq!(terms[&copper()], -1.0);
    }

    #[test]
    fn a_bundle_that_cancels_becomes_zero() {
        let zero = Quantity::resource(iron())
            .sub(&Quantity::resource(iron()))
            .unwrap();
        assert_eq!(zero.amount, Amount::Number(0.0));
    }

    #[test]
    fn a_number_and_a_bundle_do_not_add() {
        let error = Quantity::number(3.0)
            .add(&Quantity::resource(iron()))
            .unwrap_err();
        assert!(error.message().contains("number and a resource bundle"));
    }

    #[test]
    fn division_moves_a_single_term_resource_into_the_denominator() {
        let ratio = Quantity::number(6.0)
            .div(
                &Quantity::resource(iron())
                    .mul(&Quantity::number(2.0))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(ratio.amount, Amount::Number(3.0));
        assert_eq!(ratio.per, Some(iron()));
    }

    #[test]
    fn multiplying_by_the_denominator_cancels_it() {
        let ratio = Quantity::resource(copper())
            .div(
                &Quantity::resource(iron())
                    .mul(&Quantity::number(2.0))
                    .unwrap(),
            )
            .unwrap();
        let product = ratio.mul(
            &Quantity::resource(iron())
                .mul(&Quantity::number(3.0))
                .unwrap(),
        );
        let product = product.unwrap();
        assert_eq!(product.per, None);
        assert_eq!(
            product.amount,
            Amount::Resources(BTreeMap::from([(copper(), 1.5)]))
        );
    }

    #[test]
    fn two_denominators_are_unrepresentable() {
        let left = Quantity::number(1.0)
            .div(&Quantity::resource(iron()))
            .unwrap();
        let right = Quantity::number(1.0)
            .div(&Quantity::resource(copper()))
            .unwrap();
        assert!(left.mul(&right).is_err());
    }

    #[test]
    fn a_product_of_two_bundles_is_unrepresentable() {
        assert!(
            Quantity::resource(iron())
                .mul(&Quantity::resource(copper()))
                .is_err()
        );
    }

    #[test]
    fn exponents_add_and_subtract() {
        let rate = Quantity::resource(iron())
            .div(&Quantity::time(1.0))
            .unwrap();
        assert_eq!(rate.second, -1);
        let amount = rate.mul(&Quantity::time(4.0)).unwrap();
        assert_eq!(amount.second, 0);
        assert_eq!(
            amount.amount,
            Amount::Resources(BTreeMap::from([(iron(), 4.0)]))
        );
    }

    #[test]
    fn powers_require_a_number_quantity() {
        let squared = Quantity::time(2.0).pow(&Quantity::number(2.0)).unwrap();
        assert_eq!(squared.amount, Amount::Number(4.0));
        assert_eq!(squared.second, 2);
        assert!(
            Quantity::resource(iron())
                .pow(&Quantity::number(2.0))
                .is_err()
        );
        assert!(Quantity::time(2.0).pow(&Quantity::time(1.0)).is_err());
    }

    #[test]
    fn quality_filters_keep_matching_items_and_drop_fluids() {
        let rare = iron().with_quality(Quality::Rare);
        let quantity = Quantity {
            amount: Amount::Resources(BTreeMap::from([
                (iron(), 1.0),
                (rare, 2.0),
                (Resource::Fluid(EntryId::from_index(3)), 5.0),
            ])),
            per: None,
            second: 0,
            joule: 0,
            craft: 0,
        };
        let filtered = quantity.filter_quality(Comparison::GreaterEqual, Quality::Rare);
        assert_eq!(
            filtered.amount,
            Amount::Resources(BTreeMap::from([(rare, 2.0)]))
        );
    }

    #[test]
    fn quality_aliases_resolve() {
        assert_eq!(Quality::from_name("common"), Some(Quality::Normal));
        assert_eq!(Quality::from_name("quality-5"), Some(Quality::Legendary));
        assert_eq!(Quality::from_name("quality-9"), None);
        assert!((Quality::Legendary.multiplier() - 2.5).abs() < 1e-12);
    }

    #[test]
    fn a_quality_on_the_right_of_juxtaposition_is_an_error() {
        let error = Value::number(1.0)
            .juxtapose(&Value::Quality(Quality::Rare), &|_| None, &|_| Ok(()))
            .unwrap_err();
        assert!(error.message().contains("left operand"));
    }
}
