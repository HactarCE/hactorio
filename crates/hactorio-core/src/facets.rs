//! Facet-based value model.
//!
//! This module is the new value representation described by
//! `2026-10-22_design_doc_2.md`. The old single-monomial [`crate::value::Value`]
//! remains in place while the parser and evaluator are migrated.

use std::collections::{BTreeMap, BTreeSet};

use hactorio_data::{EntryId, RecipeId};
use thiserror::Error;

use crate::value::BaseUnit;

/// A stack cap: the item's own stack size, or an explicit limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cap {
    Full,
    Limit(u32),
}

/// The medium of a rate or capacity facet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    Item,
    Fluid,
}

/// Why a facet value operation is not defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FacetError {
    #[error("cannot add or subtract values with different units")]
    IncompatibleUnits,
    #[error("an exponent must be dimensionless")]
    DimensionalExponent,
    #[error("a value with units cannot be raised to a fractional power")]
    FractionalExponent,
    #[error("exponent is out of range")]
    ExponentOverflow,
    #[error("a value must have at least one facet")]
    EmptyValue,
    #[error("a process cannot coexist with any other facet")]
    ProcessExclusive,
    #[error("a machine carries its config; a separate config facet is invalid")]
    MachineHasConfig,
    #[error("a belt facet requires a rate facet")]
    BeltWithoutRate,
    #[error("no facet survives the operation")]
    NoFacetSurvives,
    #[error("process extents must both be solved and of the same kind")]
    ProcessExtentMismatch,
    #[error("process power/energy fields do not match the extent")]
    ProcessFieldMismatch,
    #[error("config count overflow")]
    CountOverflow,
    #[error("these config shapes cannot be juxtaposed")]
    ConfigJuxtaposition,
    #[error("operation is not supported for these facets")]
    UnsupportedOperation,
    #[error("cannot apply a rate or capacity to this item/fluid")]
    RateCapacityMismatch,
    #[error("an item stack size is required for this rate/capacity")]
    MissingStackSize,
    #[error("isolated rate/capacity division requires one term per side")]
    MultiTermRate,
    #[error(
        "process time arithmetic needs a solved or unsolved machine process and a time operand"
    )]
    ProcessTimeOperand,
}

/// An item monomial: a coefficient plus an integer exponent per base unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Monomial {
    coef: f64,
    units: BTreeMap<BaseUnit, i64>,
}

impl Monomial {
    pub fn scalar(coef: f64) -> Self {
        Monomial {
            coef,
            units: BTreeMap::new(),
        }
    }

    pub fn new(coef: f64, units: impl IntoIterator<Item = (BaseUnit, i64)>) -> Self {
        let mut map = BTreeMap::new();
        for (unit, exponent) in units {
            *map.entry(unit).or_insert(0) += exponent;
        }
        map.retain(|_, exponent| *exponent != 0);
        Monomial { coef, units: map }
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

    pub fn neg(&self) -> Self {
        Monomial {
            coef: -self.coef,
            units: self.units.clone(),
        }
    }

    pub fn mul(&self, other: &Monomial) -> Self {
        let mut units = self.units.clone();
        for (unit, exponent) in &other.units {
            *units.entry(*unit).or_insert(0) += exponent;
        }
        units.retain(|_, exponent| *exponent != 0);
        Monomial {
            coef: self.coef * other.coef,
            units,
        }
    }

    pub fn div(&self, other: &Monomial) -> Self {
        let mut units = self.units.clone();
        for (unit, exponent) in &other.units {
            *units.entry(*unit).or_insert(0) -= exponent;
        }
        units.retain(|_, exponent| *exponent != 0);
        Monomial {
            coef: self.coef / other.coef,
            units,
        }
    }

    pub fn add(&self, other: &Monomial) -> Result<Self, FacetError> {
        if self.units != other.units {
            return Err(FacetError::IncompatibleUnits);
        }
        Ok(Monomial {
            coef: self.coef + other.coef,
            units: self.units.clone(),
        })
    }

    pub fn sub(&self, other: &Monomial) -> Result<Self, FacetError> {
        if self.units != other.units {
            return Err(FacetError::IncompatibleUnits);
        }
        Ok(Monomial {
            coef: self.coef - other.coef,
            units: self.units.clone(),
        })
    }

    pub fn pow(&self, exponent: &Monomial) -> Result<Self, FacetError> {
        if !exponent.is_dimensionless() {
            return Err(FacetError::DimensionalExponent);
        }
        let power = exponent.coef;
        if self.is_dimensionless() {
            return Ok(Monomial::scalar(self.coef.powf(power)));
        }
        if !is_exact_integer(power) {
            return Err(FacetError::FractionalExponent);
        }
        let power_i64 = power as i64;
        if !is_exact_integer(power) || (power_i64 as f64) != power {
            return Err(FacetError::ExponentOverflow);
        }
        let mut units = BTreeMap::new();
        for (unit, unit_exponent) in &self.units {
            let result = unit_exponent
                .checked_mul(power_i64)
                .ok_or(FacetError::ExponentOverflow)?;
            if result != 0 {
                units.insert(*unit, result);
            }
        }
        Ok(Monomial {
            coef: self.coef.powf(power),
            units,
        })
    }
}

fn is_exact_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0
}

/// A stack-sized rate map. Item rates use stack caps; fluid rates use volume.
#[derive(Debug, Clone, PartialEq)]
pub enum RateFacet {
    Items(BTreeMap<Cap, f64>),
    Fluid(f64),
}

impl RateFacet {
    pub fn medium(&self) -> Medium {
        match self {
            RateFacet::Items(_) => Medium::Item,
            RateFacet::Fluid(_) => Medium::Fluid,
        }
    }
}

/// A stack-sized capacity map. Item capacities use stack caps; fluids use volume.
#[derive(Debug, Clone, PartialEq)]
pub enum CapacityFacet {
    Items(BTreeMap<Cap, f64>),
    Fluid(f64),
}

impl CapacityFacet {
    pub fn medium(&self) -> Medium {
        match self {
            CapacityFacet::Items(_) => Medium::Item,
            CapacityFacet::Fluid(_) => Medium::Fluid,
        }
    }
}

/// A belt tile count and its stacking cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltFacet {
    pub count: f64,
    pub cap: Cap,
}

/// One module count in a config facet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCount {
    pub module: EntryId,
    pub count: u32,
}

/// A beacon group: a beacon count and the modules installed in each beacon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaconGroup {
    pub beacon: EntryId,
    pub count: u32,
    pub modules: Vec<ModuleCount>,
}

/// Per-machine modules and beacon groups.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub modules: Vec<ModuleCount>,
    pub beacons: Vec<BeaconGroup>,
}

impl Config {
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty() && self.beacons.is_empty()
    }

    pub fn beacon_group_count(&self) -> usize {
        self.beacons.len()
    }

    pub fn is_module_only(&self) -> bool {
        self.beacons.is_empty()
    }

    /// Cb: exactly one beacon group.
    pub fn single_beacon_group(&self) -> Option<&BeaconGroup> {
        match self.beacons.as_slice() {
            [group] => Some(group),
            _ => None,
        }
    }

    pub fn merge_add(&mut self, other: &Config) -> Result<(), FacetError> {
        let merged = merge_config(self, other)?;
        *self = merged;
        Ok(())
    }

    /// Partial config juxtaposition (§3.6).
    pub fn juxtapose(&self, other: &Config) -> Result<ConfigJuxtaposition, FacetError> {
        if self.single_beacon_group().is_some() {
            if other.is_module_only() {
                let mut out = self.clone();
                let out_group = &mut out.beacons[0];
                merge_modules(&mut out_group.modules, &other.modules)?;
                out.canonicalize();
                return Ok(ConfigJuxtaposition::Attach(out));
            }
            return Err(FacetError::ConfigJuxtaposition);
        }
        if self.is_module_only() && other.is_module_only() {
            let mut out = self.clone();
            merge_modules(&mut out.modules, &other.modules)?;
            out.canonicalize();
            return Ok(ConfigJuxtaposition::Modules(out));
        }
        Err(FacetError::ConfigJuxtaposition)
    }

    /// Canonical ordering: modules by id; beacon groups by beacon id then module set.
    pub fn canonicalize(&mut self) {
        self.modules.retain(|module| module.count != 0);
        self.modules.sort_by_key(|module| module.module);

        for group in &mut self.beacons {
            group.modules.retain(|module| module.count != 0);
            group.modules.sort_by_key(|module| module.module);
        }
        self.beacons.retain(|group| group.count != 0);
        self.beacons.sort_by(|a, b| {
            a.beacon
                .cmp(&b.beacon)
                .then_with(|| module_key(&a.modules).cmp(&module_key(&b.modules)))
        });
    }
}

fn module_key(modules: &[ModuleCount]) -> Vec<(EntryId, u32)> {
    modules
        .iter()
        .map(|module| (module.module, module.count))
        .collect()
}

/// The result of partial config juxtaposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigJuxtaposition {
    /// `Cb Cm`: attach RHS modules to the single beacon group; keep LHS item.
    Attach(Config),
    /// `Cm Cm`: combine module lists; both item facets are consumed.
    Modules(Config),
}

/// A machine plus its per-machine config and optional explicit count.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineFacet {
    pub machine: EntryId,
    pub config: Config,
    /// `None` means an uncounted machine specification.
    pub count: Option<f64>,
}

/// A process extent: machine count or machine-seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Extent {
    Machines(f64),
    MachineSeconds(f64),
}

impl Extent {
    pub fn magnitude(self) -> f64 {
        match self {
            Extent::Machines(value) | Extent::MachineSeconds(value) => value,
        }
    }
}

/// A production process. Effects are baked; modules/beacons are not retained.
#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    /// `None` means an unsolved process template.
    pub extent: Option<Extent>,
    pub net: BTreeMap<BaseUnit, f64>,
    pub power_w: Option<f64>,
    pub energy_j: Option<f64>,
    pub recipes: BTreeSet<RecipeId>,
}

/// A facet value with independent optional facets.
#[derive(Debug, Clone, PartialEq)]
pub struct FacetValue {
    pub item: Option<Monomial>,
    pub recipe: Option<RecipeId>,
    pub machine: Option<MachineFacet>,
    pub process: Option<Process>,
    pub config: Option<Config>,
    pub belt: Option<BeltFacet>,
    pub rate: Option<RateFacet>,
    pub capacity: Option<CapacityFacet>,
}

impl FacetValue {
    pub fn scalar(coef: f64) -> Self {
        FacetValue {
            item: Some(Monomial::scalar(coef)),
            recipe: None,
            machine: None,
            process: None,
            config: None,
            belt: None,
            rate: None,
            capacity: None,
        }
    }

    pub fn item(item: Monomial) -> Self {
        FacetValue {
            item: Some(item),
            ..Self::empty()
        }
    }

    pub fn empty() -> Self {
        FacetValue {
            item: None,
            recipe: None,
            machine: None,
            process: None,
            config: None,
            belt: None,
            rate: None,
            capacity: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.item.is_none()
            && self.recipe.is_none()
            && self.machine.is_none()
            && self.process.is_none()
            && self.config.is_none()
            && self.belt.is_none()
            && self.rate.is_none()
            && self.capacity.is_none()
    }

    pub fn abs(&self) -> Result<Self, FacetError> {
        let mut out = FacetValue::empty();
        out.item = self.item.as_ref().map(|item| {
            Monomial::new(
                item.coef().abs(),
                item.units()
                    .iter()
                    .map(|(unit, exponent)| (*unit, *exponent)),
            )
        });
        out.machine = self.machine.as_ref().map(|machine| MachineFacet {
            machine: machine.machine,
            config: machine.config.clone(),
            count: machine.count.map(f64::abs),
        });
        out.process = self.process.as_ref().map(|process| Process {
            extent: process.extent.map(|extent| match extent {
                Extent::Machines(value) => Extent::Machines(value.abs()),
                Extent::MachineSeconds(value) => Extent::MachineSeconds(value.abs()),
            }),
            net: process
                .net
                .iter()
                .map(|(unit, amount)| (*unit, amount.abs()))
                .collect(),
            power_w: process.power_w.map(f64::abs),
            energy_j: process.energy_j.map(f64::abs),
            recipes: process.recipes.clone(),
        });
        out.config = self.config.clone();
        out.belt = self.belt.map(|belt| BeltFacet {
            count: belt.count.abs(),
            cap: belt.cap,
        });
        out.rate = self.rate.as_ref().map(abs_rate);
        out.capacity = self.capacity.as_ref().map(abs_capacity);
        if out.is_empty() {
            return Err(FacetError::NoFacetSurvives);
        }
        Ok(out)
    }

    pub fn add(&self, other: &Self) -> Result<Self, FacetError> {
        self.combine(other, false)
    }

    pub fn sub(&self, other: &Self) -> Result<Self, FacetError> {
        self.combine(other, true)
    }

    fn combine(&self, other: &Self, subtract: bool) -> Result<Self, FacetError> {
        let mut out = FacetValue::empty();

        out.item = match (&self.item, &other.item) {
            (Some(a), Some(b)) => {
                let result = if subtract { a.sub(b) } else { a.add(b) };
                result.ok()
            }
            _ => None,
        };

        out.machine = match (&self.machine, &other.machine) {
            (Some(a), Some(b)) if a.machine == b.machine && a.config == b.config => {
                let a_count = a.count.unwrap_or(1.0);
                let b_count = b.count.unwrap_or(1.0);
                Some(MachineFacet {
                    machine: a.machine,
                    config: a.config.clone(),
                    count: Some(if subtract {
                        a_count - b_count
                    } else {
                        a_count + b_count
                    }),
                })
            }
            _ => None,
        };

        out.process = match (&self.process, &other.process) {
            (Some(a), Some(b)) => Some(combine_process(a, b, subtract)?),
            _ => None,
        };

        out.config = if subtract {
            None
        } else {
            match (&self.config, &other.config) {
                (Some(a), Some(b)) => Some(merge_config(a, b)?),
                _ => None,
            }
        };

        out.belt = match (&self.belt, &other.belt) {
            (Some(a), Some(b)) if a.cap == b.cap => Some(BeltFacet {
                count: if subtract {
                    a.count - b.count
                } else {
                    a.count + b.count
                },
                cap: a.cap,
            }),
            _ => None,
        };

        out.rate = match (&self.rate, &other.rate) {
            (Some(a), Some(b)) => combine_rate(a, b, subtract),
            _ => None,
        };

        out.capacity = match (&self.capacity, &other.capacity) {
            (Some(a), Some(b)) => combine_capacity(a, b, subtract),
            _ => None,
        };

        if out.is_empty() {
            return Err(FacetError::NoFacetSurvives);
        }
        Ok(out)
    }

    pub fn neg(&self) -> Result<Self, FacetError> {
        let mut out = FacetValue::empty();
        out.item = self.item.as_ref().map(Monomial::neg);
        out.machine = self.machine.as_ref().map(|machine| MachineFacet {
            machine: machine.machine,
            config: machine.config.clone(),
            count: Some(-machine.count.unwrap_or(1.0)),
        });
        out.process = self.process.as_ref().map(|process| {
            let extent = match process.extent {
                Some(extent) => Some(match extent {
                    Extent::Machines(value) => Extent::Machines(-value),
                    Extent::MachineSeconds(value) => Extent::MachineSeconds(-value),
                }),
                None => Some(Extent::Machines(-1.0)),
            };
            Process {
                extent,
                net: negate_map(&process.net),
                power_w: process.power_w.map(|value| -value),
                energy_j: process.energy_j.map(|value| -value),
                recipes: process.recipes.clone(),
            }
        });
        out.belt = self.belt.map(|belt| BeltFacet {
            count: -belt.count,
            cap: belt.cap,
        });
        out.rate = self.rate.as_ref().map(negate_rate);
        out.capacity = self.capacity.as_ref().map(negate_capacity);
        if out.is_empty() {
            return Err(FacetError::NoFacetSurvives);
        }
        Ok(out)
    }

    fn scalar_value(&self) -> Option<f64> {
        if self.recipe.is_none()
            && self.machine.is_none()
            && self.process.is_none()
            && self.config.is_none()
            && self.belt.is_none()
            && self.rate.is_none()
            && self.capacity.is_none()
        {
            self.item
                .as_ref()
                .filter(|item| item.is_dimensionless())
                .map(|item| item.coef())
        } else {
            None
        }
    }

    fn time_amount(&self) -> Option<f64> {
        if self.recipe.is_none()
            && self.machine.is_none()
            && self.process.is_none()
            && self.config.is_none()
            && self.belt.is_none()
            && self.rate.is_none()
            && self.capacity.is_none()
        {
            let item = self.item.as_ref()?;
            if item.units().len() == 1 && item.exponent(BaseUnit::Second) == 1 {
                return Some(item.coef());
            }
        }
        None
    }

    fn pure_item_amount(&self) -> Option<(BaseUnit, f64)> {
        let item = self.item.as_ref()?;
        if item.units().len() != 1 {
            return None;
        }
        let (unit, exponent) = item.units().iter().next()?;
        if *exponent != 1 {
            return None;
        }
        if !matches!(unit, BaseUnit::Item(_) | BaseUnit::Fluid(_)) {
            return None;
        }
        Some((*unit, item.coef()))
    }

    fn is_isolated_rate(&self) -> bool {
        self.rate.is_some()
            && self.item.is_none()
            && self.recipe.is_none()
            && self.machine.is_none()
            && self.process.is_none()
            && self.config.is_none()
            && self.belt.is_none()
            && self.capacity.is_none()
    }

    fn is_isolated_capacity(&self) -> bool {
        self.capacity.is_some()
            && self.item.is_none()
            && self.recipe.is_none()
            && self.machine.is_none()
            && self.process.is_none()
            && self.config.is_none()
            && self.belt.is_none()
            && self.rate.is_none()
    }

    pub fn mul(&self, other: &Self) -> Result<Self, FacetError> {
        if let Some(factor) = self.scalar_value() {
            return other.scale(factor);
        }
        if let Some(factor) = other.scalar_value() {
            return self.scale(factor);
        }

        if let Some(process) = &self.process {
            return match other.time_amount() {
                Some(time) => scale_process_time(process, time),
                None => Err(FacetError::ProcessTimeOperand),
            };
        }
        if let Some(process) = &other.process {
            return match self.time_amount() {
                Some(time) => scale_process_time(process, time),
                None => Err(FacetError::ProcessTimeOperand),
            };
        }

        if self.is_isolated_rate() {
            if let Some(time) = other.time_amount() {
                return Ok(rate_to_capacity(self.rate.as_ref().unwrap(), time));
            }
        }

        if let (Some(a), Some(b)) = (&self.item, &other.item) {
            return Ok(FacetValue::item(a.mul(b)));
        }

        Err(FacetError::UnsupportedOperation)
    }

    pub fn div(&self, other: &Self) -> Result<Self, FacetError> {
        if let Some(divisor) = other.scalar_value() {
            let mut base = self.clone();
            base.config = None;
            return base.scale(1.0 / divisor);
        }

        if let Some(process) = &self.process {
            return match other.time_amount() {
                Some(time) => divide_process_time(process, time),
                None => Err(FacetError::ProcessTimeOperand),
            };
        }
        if other.process.is_some() {
            return Err(FacetError::UnsupportedOperation);
        }

        if self.is_isolated_capacity() {
            if let Some(time) = other.time_amount() {
                return Ok(scale_capacity_to_rate(
                    self.capacity.as_ref().unwrap(),
                    1.0 / time,
                ));
            }
        }
        if self.is_isolated_rate() && other.time_amount().is_some() {
            return Err(FacetError::UnsupportedOperation);
        }

        if self.is_isolated_rate() && other.is_isolated_rate() {
            return divide_isolated_rate_capacity(self, other);
        }
        if self.is_isolated_rate() && other.is_isolated_capacity() {
            return divide_isolated_rate_capacity(self, other);
        }
        if self.is_isolated_capacity() && other.is_isolated_rate() {
            return divide_isolated_rate_capacity(self, other);
        }
        if self.is_isolated_capacity() && other.is_isolated_capacity() {
            return divide_isolated_rate_capacity(self, other);
        }

        if let (Some(a), Some(b)) = (&self.item, &other.item) {
            return Ok(FacetValue::item(a.div(b)));
        }

        Err(FacetError::UnsupportedOperation)
    }

    pub fn juxtapose(
        &self,
        other: &Self,
        rhs_item_stack_size: Option<u32>,
    ) -> Result<Self, FacetError> {
        if let Some(rate) = &self.rate {
            if let Some((unit, coef)) = other.pure_item_amount() {
                return apply_rate_or_capacity(rate, unit, coef, rhs_item_stack_size);
            }
        }
        if let Some(capacity) = &self.capacity {
            if let Some((unit, coef)) = other.pure_item_amount() {
                return apply_capacity(capacity, unit, coef, rhs_item_stack_size);
            }
        }

        if let (Some(a), Some(b)) = (&self.config, &other.config) {
            return match a.juxtapose(b)? {
                ConfigJuxtaposition::Attach(config) => Ok(FacetValue {
                    item: self.item.clone(),
                    config: Some(config),
                    ..FacetValue::empty()
                }),
                ConfigJuxtaposition::Modules(config) => Ok(FacetValue {
                    config: Some(config),
                    ..FacetValue::empty()
                }),
            };
        }

        if let Some(factor) = self.scalar_value() {
            return other.scale(factor);
        }
        if let Some(factor) = other.scalar_value() {
            return self.scale(factor);
        }

        Err(FacetError::UnsupportedOperation)
    }

    pub fn pow(&self, exponent: &Self) -> Result<Self, FacetError> {
        let exponent = exponent
            .item
            .as_ref()
            .ok_or(FacetError::DimensionalExponent)?;
        let item = self.item.as_ref().ok_or(FacetError::UnsupportedOperation)?;
        Ok(FacetValue::item(item.pow(exponent)?))
    }

    fn scale(&self, factor: f64) -> Result<Self, FacetError> {
        let mut out = FacetValue::empty();
        out.item = self.item.as_ref().map(|item| {
            Monomial::new(
                item.coef() * factor,
                item.units()
                    .iter()
                    .map(|(unit, exponent)| (*unit, *exponent)),
            )
        });
        out.machine = self.machine.as_ref().map(|machine| MachineFacet {
            machine: machine.machine,
            config: machine.config.clone(),
            count: Some(machine.count.unwrap_or(1.0) * factor),
        });
        out.process = self.process.as_ref().map(|process| {
            let extent = match process.extent {
                Some(Extent::Machines(value)) => Some(Extent::Machines(value * factor)),
                Some(Extent::MachineSeconds(value)) => Some(Extent::MachineSeconds(value * factor)),
                None => Some(Extent::Machines(factor)),
            };
            Process {
                extent,
                net: process
                    .net
                    .iter()
                    .map(|(unit, amount)| (*unit, *amount * factor))
                    .collect(),
                power_w: process.power_w.map(|value| value * factor),
                energy_j: process.energy_j.map(|value| value * factor),
                recipes: process.recipes.clone(),
            }
        });
        out.config = match (&self.config, is_nonnegative_integer(factor)) {
            (Some(config), true) => {
                let mut config = config.clone();
                for module in &mut config.modules {
                    module.count = (module.count as f64 * factor) as u32;
                }
                for group in &mut config.beacons {
                    group.count = (group.count as f64 * factor) as u32;
                }
                config.canonicalize();
                Some(config)
            }
            _ => None,
        };
        out.belt = self.belt.map(|belt| BeltFacet {
            count: belt.count * factor,
            cap: belt.cap,
        });
        out.rate = self.rate.as_ref().map(|rate| scale_rate(rate, factor));
        out.capacity = self
            .capacity
            .as_ref()
            .map(|capacity| scale_capacity(capacity, factor));
        if out.is_empty() {
            return Err(FacetError::NoFacetSurvives);
        }
        Ok(out)
    }

    /// Checks the coexistence invariants from §3.1.1.
    pub fn validate(&self) -> Result<(), FacetError> {
        if self.is_empty() {
            return Err(FacetError::EmptyValue);
        }
        if self.process.is_some()
            && (self.item.is_some()
                || self.recipe.is_some()
                || self.machine.is_some()
                || self.config.is_some()
                || self.belt.is_some()
                || self.rate.is_some()
                || self.capacity.is_some())
        {
            return Err(FacetError::ProcessExclusive);
        }
        if self.machine.is_some() && self.config.is_some() {
            return Err(FacetError::MachineHasConfig);
        }
        if self.belt.is_some() && self.rate.is_none() {
            return Err(FacetError::BeltWithoutRate);
        }
        Ok(())
    }
}

fn abs_terms(map: &BTreeMap<Cap, f64>) -> BTreeMap<Cap, f64> {
    map.iter()
        .map(|(cap, amount)| (*cap, amount.abs()))
        .collect()
}

fn abs_rate(rate: &RateFacet) -> RateFacet {
    match rate {
        RateFacet::Items(items) => RateFacet::Items(abs_terms(items)),
        RateFacet::Fluid(value) => RateFacet::Fluid(value.abs()),
    }
}

fn abs_capacity(capacity: &CapacityFacet) -> CapacityFacet {
    match capacity {
        CapacityFacet::Items(items) => CapacityFacet::Items(abs_terms(items)),
        CapacityFacet::Fluid(value) => CapacityFacet::Fluid(value.abs()),
    }
}

fn negate_map(map: &BTreeMap<BaseUnit, f64>) -> BTreeMap<BaseUnit, f64> {
    map.iter().map(|(unit, amount)| (*unit, -*amount)).collect()
}

fn negate_terms(map: &BTreeMap<Cap, f64>) -> BTreeMap<Cap, f64> {
    map.iter().map(|(cap, amount)| (*cap, -*amount)).collect()
}

fn negate_rate(rate: &RateFacet) -> RateFacet {
    match rate {
        RateFacet::Items(items) => RateFacet::Items(negate_terms(items)),
        RateFacet::Fluid(value) => RateFacet::Fluid(-value),
    }
}

fn negate_capacity(capacity: &CapacityFacet) -> CapacityFacet {
    match capacity {
        CapacityFacet::Items(items) => CapacityFacet::Items(negate_terms(items)),
        CapacityFacet::Fluid(value) => CapacityFacet::Fluid(-value),
    }
}

fn is_nonnegative_integer(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value.fract() == 0.0
}

fn rate_to_capacity(rate: &RateFacet, time: f64) -> FacetValue {
    let capacity = match rate {
        RateFacet::Items(items) => CapacityFacet::Items(
            items
                .iter()
                .map(|(cap, amount)| (*cap, *amount * time))
                .collect(),
        ),
        RateFacet::Fluid(value) => CapacityFacet::Fluid(value * time),
    };
    FacetValue {
        capacity: Some(capacity),
        ..FacetValue::empty()
    }
}

fn effective_stack(cap: Cap, stack_size: u32) -> u32 {
    match cap {
        Cap::Full => stack_size,
        Cap::Limit(limit) => limit.min(stack_size),
    }
}

fn apply_rate_or_capacity(
    facet: &RateFacet,
    unit: BaseUnit,
    coef: f64,
    item_stack_size: Option<u32>,
) -> Result<FacetValue, FacetError> {
    let amount = match (facet, unit) {
        (RateFacet::Fluid(value), BaseUnit::Fluid(_)) => value * coef,
        (RateFacet::Items(items), BaseUnit::Item(_)) => {
            let stack_size = item_stack_size.ok_or(FacetError::MissingStackSize)?;
            items
                .iter()
                .map(|(cap, value)| *value * f64::from(effective_stack(*cap, stack_size)))
                .sum::<f64>()
                * coef
        }
        _ => return Err(FacetError::RateCapacityMismatch),
    };
    Ok(FacetValue::item(Monomial::new(amount, [(unit, 1)])))
}

fn apply_capacity(
    facet: &CapacityFacet,
    unit: BaseUnit,
    coef: f64,
    item_stack_size: Option<u32>,
) -> Result<FacetValue, FacetError> {
    let amount = match (facet, unit) {
        (CapacityFacet::Fluid(value), BaseUnit::Fluid(_)) => value * coef,
        (CapacityFacet::Items(items), BaseUnit::Item(_)) => {
            let stack_size = item_stack_size.ok_or(FacetError::MissingStackSize)?;
            items
                .iter()
                .map(|(cap, value)| *value * f64::from(effective_stack(*cap, stack_size)))
                .sum::<f64>()
                * coef
        }
        _ => return Err(FacetError::RateCapacityMismatch),
    };
    Ok(FacetValue::item(Monomial::new(amount, [(unit, 1)])))
}

enum AmountFacet<'a> {
    Items(&'a BTreeMap<Cap, f64>),
    Fluid(f64),
}

fn rate_amount(rate: &RateFacet) -> AmountFacet<'_> {
    match rate {
        RateFacet::Items(items) => AmountFacet::Items(items),
        RateFacet::Fluid(value) => AmountFacet::Fluid(*value),
    }
}

fn capacity_amount(capacity: &CapacityFacet) -> AmountFacet<'_> {
    match capacity {
        CapacityFacet::Items(items) => AmountFacet::Items(items),
        CapacityFacet::Fluid(value) => AmountFacet::Fluid(*value),
    }
}

fn divide_amounts(a: AmountFacet<'_>, b: AmountFacet<'_>) -> Result<f64, FacetError> {
    match (a, b) {
        (AmountFacet::Fluid(a), AmountFacet::Fluid(b)) => Ok(a / b),
        (AmountFacet::Items(a), AmountFacet::Items(b)) => {
            if a.len() > 1 || b.len() > 1 {
                return Err(FacetError::MultiTermRate);
            }
            if b.is_empty() {
                let a_value = a
                    .values()
                    .next()
                    .copied()
                    .ok_or(FacetError::MultiTermRate)?;
                return Ok(a_value / 0.0);
            }
            let (a_cap, a_value) = a.iter().next().unwrap();
            let (b_cap, b_value) = b.iter().next().unwrap();
            if a_cap != b_cap {
                return Err(FacetError::RateCapacityMismatch);
            }
            Ok(a_value / b_value)
        }
        _ => Err(FacetError::RateCapacityMismatch),
    }
}

fn divide_isolated_rate_capacity(a: &FacetValue, b: &FacetValue) -> Result<FacetValue, FacetError> {
    let (value, unit) = match (
        a.rate.as_ref(),
        a.capacity.as_ref(),
        b.rate.as_ref(),
        b.capacity.as_ref(),
    ) {
        (Some(ar), None, Some(br), None) => {
            (divide_amounts(rate_amount(ar), rate_amount(br))?, None)
        }
        (None, Some(ac), None, Some(bc)) => (
            divide_amounts(capacity_amount(ac), capacity_amount(bc))?,
            None,
        ),
        (Some(ar), None, None, Some(bc)) => (
            divide_amounts(rate_amount(ar), capacity_amount(bc))?,
            Some((BaseUnit::Second, -1)),
        ),
        (None, Some(ac), Some(br), None) => (
            divide_amounts(capacity_amount(ac), rate_amount(br))?,
            Some((BaseUnit::Second, 1)),
        ),
        _ => return Err(FacetError::RateCapacityMismatch),
    };
    Ok(match unit {
        None => FacetValue::scalar(value),
        Some((unit, exponent)) => FacetValue::item(Monomial::new(value, [(unit, exponent)])),
    })
}

fn scale_rate(rate: &RateFacet, factor: f64) -> RateFacet {
    match rate {
        RateFacet::Items(items) => RateFacet::Items(
            items
                .iter()
                .map(|(cap, amount)| (*cap, *amount * factor))
                .collect(),
        ),
        RateFacet::Fluid(value) => RateFacet::Fluid(value * factor),
    }
}

fn scale_capacity(capacity: &CapacityFacet, factor: f64) -> CapacityFacet {
    match capacity {
        CapacityFacet::Items(items) => CapacityFacet::Items(
            items
                .iter()
                .map(|(cap, amount)| (*cap, *amount * factor))
                .collect(),
        ),
        CapacityFacet::Fluid(value) => CapacityFacet::Fluid(value * factor),
    }
}

fn scale_capacity_to_rate(capacity: &CapacityFacet, factor: f64) -> FacetValue {
    let rate = match capacity {
        CapacityFacet::Items(items) => RateFacet::Items(
            items
                .iter()
                .map(|(cap, amount)| (*cap, *amount * factor))
                .collect(),
        ),
        CapacityFacet::Fluid(value) => RateFacet::Fluid(value * factor),
    };
    FacetValue {
        rate: Some(rate),
        ..FacetValue::empty()
    }
}

fn scale_process_time(process: &Process, time: f64) -> Result<FacetValue, FacetError> {
    let (extent, net) = match process.extent {
        Some(Extent::Machines(count)) => (
            Extent::MachineSeconds(count * time),
            process
                .net
                .iter()
                .map(|(unit, amount)| (*unit, *amount * time))
                .collect(),
        ),
        None => (
            Extent::MachineSeconds(time),
            process
                .net
                .iter()
                .map(|(unit, amount)| (*unit, *amount * time))
                .collect(),
        ),
        Some(Extent::MachineSeconds(_)) => return Err(FacetError::ProcessTimeOperand),
    };
    let energy_j = process.power_w.map(|power| power * time);
    Ok(FacetValue {
        process: Some(Process {
            extent: Some(extent),
            net,
            power_w: None,
            energy_j,
            recipes: process.recipes.clone(),
        }),
        ..FacetValue::empty()
    })
}

fn divide_process_time(process: &Process, time: f64) -> Result<FacetValue, FacetError> {
    let Extent::MachineSeconds(tau) = process.extent.ok_or(FacetError::ProcessTimeOperand)? else {
        return Err(FacetError::ProcessTimeOperand);
    };
    let net = process
        .net
        .iter()
        .map(|(unit, amount)| (*unit, *amount / time))
        .collect();
    let power_w = process.energy_j.map(|energy| energy / time);
    Ok(FacetValue {
        process: Some(Process {
            extent: Some(Extent::Machines(tau / time)),
            net,
            power_w,
            energy_j: None,
            recipes: process.recipes.clone(),
        }),
        ..FacetValue::empty()
    })
}

fn combine_process(a: &Process, b: &Process, subtract: bool) -> Result<Process, FacetError> {
    let (a_extent, b_extent) = match (a.extent, b.extent) {
        (Some(a_extent), Some(b_extent)) => (a_extent, b_extent),
        _ => return Err(FacetError::ProcessExtentMismatch),
    };
    let extent = match (a_extent, b_extent) {
        (Extent::Machines(a_value), Extent::Machines(b_value)) => Extent::Machines(if subtract {
            a_value - b_value
        } else {
            a_value + b_value
        }),
        (Extent::MachineSeconds(a_value), Extent::MachineSeconds(b_value)) => {
            Extent::MachineSeconds(if subtract {
                a_value - b_value
            } else {
                a_value + b_value
            })
        }
        _ => return Err(FacetError::ProcessExtentMismatch),
    };

    let recipes = if subtract {
        if a.recipes.len() == 1 && a.recipes == b.recipes {
            a.recipes.clone()
        } else {
            BTreeSet::new()
        }
    } else {
        a.recipes.union(&b.recipes).copied().collect()
    };

    Ok(Process {
        extent: Some(extent),
        net: combine_amounts(&a.net, &b.net, subtract),
        power_w: combine_option(a.power_w, b.power_w, subtract)?,
        energy_j: combine_option(a.energy_j, b.energy_j, subtract)?,
        recipes,
    })
}

fn combine_option(
    a: Option<f64>,
    b: Option<f64>,
    subtract: bool,
) -> Result<Option<f64>, FacetError> {
    match (a, b) {
        (Some(a), Some(b)) => Ok(Some(if subtract { a - b } else { a + b })),
        (None, None) => Ok(None),
        _ => Err(FacetError::ProcessFieldMismatch),
    }
}

fn combine_amounts(
    a: &BTreeMap<BaseUnit, f64>,
    b: &BTreeMap<BaseUnit, f64>,
    subtract: bool,
) -> BTreeMap<BaseUnit, f64> {
    let mut out = a.clone();
    for (unit, amount) in b {
        let entry = out.entry(*unit).or_insert(0.0);
        if subtract {
            *entry -= amount;
        } else {
            *entry += amount;
        }
    }
    out.retain(|_, amount| *amount != 0.0);
    out
}

fn merge_modules(target: &mut Vec<ModuleCount>, source: &[ModuleCount]) -> Result<(), FacetError> {
    for module in source {
        match target
            .iter_mut()
            .find(|entry| entry.module == module.module)
        {
            Some(entry) => {
                entry.count = entry
                    .count
                    .checked_add(module.count)
                    .ok_or(FacetError::CountOverflow)?;
            }
            None => target.push(module.clone()),
        }
    }
    target.sort_by_key(|module| module.module);
    Ok(())
}

fn merge_config(a: &Config, b: &Config) -> Result<Config, FacetError> {
    let mut out = a.clone();
    out.canonicalize();
    let mut b = b.clone();
    b.canonicalize();
    for module in &b.modules {
        match out
            .modules
            .iter_mut()
            .find(|entry| entry.module == module.module)
        {
            Some(entry) => {
                entry.count = entry
                    .count
                    .checked_add(module.count)
                    .ok_or(FacetError::CountOverflow)?;
            }
            None => out.modules.push(module.clone()),
        }
    }
    for group in &b.beacons {
        match out.beacons.iter_mut().find(|entry| {
            entry.beacon == group.beacon && same_modules(&entry.modules, &group.modules)
        }) {
            Some(entry) => {
                entry.count = entry
                    .count
                    .checked_add(group.count)
                    .ok_or(FacetError::CountOverflow)?;
            }
            None => out.beacons.push(group.clone()),
        }
    }
    out.canonicalize();
    Ok(out)
}

fn same_modules(a: &[ModuleCount], b: &[ModuleCount]) -> bool {
    module_key(a) == module_key(b)
}

fn combine_rate(a: &RateFacet, b: &RateFacet, subtract: bool) -> Option<RateFacet> {
    match (a, b) {
        (RateFacet::Items(a), RateFacet::Items(b)) => {
            Some(RateFacet::Items(combine_terms(a, b, subtract)))
        }
        (RateFacet::Fluid(a), RateFacet::Fluid(b)) => {
            Some(RateFacet::Fluid(if subtract { a - b } else { a + b }))
        }
        _ => None,
    }
}

fn combine_capacity(a: &CapacityFacet, b: &CapacityFacet, subtract: bool) -> Option<CapacityFacet> {
    match (a, b) {
        (CapacityFacet::Items(a), CapacityFacet::Items(b)) => {
            Some(CapacityFacet::Items(combine_terms(a, b, subtract)))
        }
        (CapacityFacet::Fluid(a), CapacityFacet::Fluid(b)) => {
            Some(CapacityFacet::Fluid(if subtract { a - b } else { a + b }))
        }
        _ => None,
    }
}

fn combine_terms(
    a: &BTreeMap<Cap, f64>,
    b: &BTreeMap<Cap, f64>,
    subtract: bool,
) -> BTreeMap<Cap, f64> {
    let mut out = a.clone();
    for (cap, amount) in b {
        let entry = out.entry(*cap).or_insert(0.0);
        if subtract {
            *entry -= amount;
        } else {
            *entry += amount;
        }
    }
    out.retain(|_, amount| *amount != 0.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_is_an_empty_item_facet() {
        let value = FacetValue::scalar(3.0);
        assert_eq!(value.item, Some(Monomial::scalar(3.0)));
        assert_eq!(value.item.as_ref().unwrap().units().len(), 0);
        assert!(value.validate().is_ok());
    }

    #[test]
    fn zero_exponents_are_canonicalized() {
        let item = EntryId::from_index(0);
        let monomial = Monomial::new(1.0, [(BaseUnit::Item(item), 1), (BaseUnit::Item(item), -1)]);
        assert!(monomial.units().is_empty());
    }

    #[test]
    fn process_cannot_coexist_with_other_facets() {
        let mut value = FacetValue::empty();
        value.process = Some(Process {
            extent: None,
            net: BTreeMap::new(),
            power_w: None,
            energy_j: None,
            recipes: BTreeSet::new(),
        });
        assert!(value.validate().is_ok());
        value.item = Some(Monomial::scalar(1.0));
        assert_eq!(value.validate(), Err(FacetError::ProcessExclusive));
    }

    #[test]
    fn belt_requires_rate() {
        let mut value = FacetValue::item(Monomial::scalar(1.0));
        value.belt = Some(BeltFacet {
            count: 1.0,
            cap: Cap::Limit(1),
        });
        assert_eq!(value.validate(), Err(FacetError::BeltWithoutRate));
        value.rate = Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(1), 15.0)])));
        assert!(value.validate().is_ok());
    }

    #[test]
    fn item_addition_drops_recipe() {
        let item = EntryId::from_index(0);
        let mut a = FacetValue::item(Monomial::new(2.0, [(BaseUnit::Item(item), 1)]));
        a.recipe = Some(RecipeId::from_index(0));
        let b = FacetValue::item(Monomial::new(3.0, [(BaseUnit::Item(item), 1)]));
        let result = a.add(&b).unwrap();
        assert_eq!(result.item.unwrap().coef(), 5.0);
        assert!(result.recipe.is_none());
    }

    #[test]
    fn incompatible_items_leave_no_facet() {
        let a = FacetValue::item(Monomial::new(
            1.0,
            [(BaseUnit::Item(EntryId::from_index(0)), 1)],
        ));
        let b = FacetValue::item(Monomial::new(
            1.0,
            [(BaseUnit::Item(EntryId::from_index(1)), 1)],
        ));
        assert_eq!(a.add(&b), Err(FacetError::NoFacetSurvives));
    }

    #[test]
    fn config_addition_merges_modules_and_beacons() {
        let module = EntryId::from_index(2);
        let beacon = EntryId::from_index(3);
        let mut a = FacetValue::empty();
        a.config = Some(Config {
            modules: vec![ModuleCount { module, count: 2 }],
            beacons: vec![BeaconGroup {
                beacon,
                count: 1,
                modules: vec![ModuleCount { module, count: 2 }],
            }],
        });
        let mut b = FacetValue::empty();
        b.config = Some(Config {
            modules: vec![ModuleCount { module, count: 3 }],
            beacons: vec![BeaconGroup {
                beacon,
                count: 4,
                modules: vec![ModuleCount { module, count: 2 }],
            }],
        });
        let result = a.add(&b).unwrap();
        let config = result.config.unwrap();
        assert_eq!(config.modules[0].count, 5);
        assert_eq!(config.beacons[0].count, 5);
    }

    #[test]
    fn belt_addition_keeps_rate_and_cap() {
        let item = EntryId::from_index(0);
        let mut a = FacetValue::item(Monomial::new(1.0, [(BaseUnit::Item(item), 1)]));
        a.belt = Some(BeltFacet {
            count: 1.0,
            cap: Cap::Limit(1),
        });
        a.rate = Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(1), 15.0)])));
        let result = a.add(&a).unwrap();
        assert_eq!(result.belt.unwrap().count, 2.0);
        assert_eq!(result.rate.unwrap().medium(), Medium::Item);
    }

    #[test]
    fn process_addition_unions_recipes() {
        let mut a = FacetValue::empty();
        a.process = Some(Process {
            extent: Some(Extent::Machines(1.0)),
            net: BTreeMap::from([(BaseUnit::Second, -1.0)]),
            power_w: Some(10.0),
            energy_j: None,
            recipes: BTreeSet::from([RecipeId::from_index(0)]),
        });
        let mut b = FacetValue::empty();
        b.process = Some(Process {
            extent: Some(Extent::Machines(2.0)),
            net: BTreeMap::from([(BaseUnit::Second, -2.0)]),
            power_w: Some(20.0),
            energy_j: None,
            recipes: BTreeSet::from([RecipeId::from_index(1)]),
        });
        let result = a.add(&b).unwrap().process.unwrap();
        assert_eq!(result.extent, Some(Extent::Machines(3.0)));
        assert_eq!(result.recipes.len(), 2);
        assert_eq!(result.power_w, Some(30.0));
    }

    #[test]
    fn negation_solves_an_unsolved_process() {
        let mut value = FacetValue::empty();
        value.process = Some(Process {
            extent: None,
            net: BTreeMap::from([(BaseUnit::Second, 1.0)]),
            power_w: Some(5.0),
            energy_j: None,
            recipes: BTreeSet::new(),
        });
        let process = value.neg().unwrap().process.unwrap();
        assert_eq!(process.extent, Some(Extent::Machines(-1.0)));
        assert_eq!(process.net[&BaseUnit::Second], -1.0);
        assert_eq!(process.power_w, Some(-5.0));
    }

    #[test]
    fn negation_drops_config_and_recipe() {
        let item = EntryId::from_index(0);
        let mut value = FacetValue::item(Monomial::new(2.0, [(BaseUnit::Item(item), 1)]));
        value.recipe = Some(RecipeId::from_index(0));
        value.config = Some(Config::default());
        let result = value.neg().unwrap();
        assert_eq!(result.item.unwrap().coef(), -2.0);
        assert!(result.recipe.is_none());
        assert!(result.config.is_none());
    }

    #[test]
    fn scalar_scales_item_and_machine_count() {
        let item = EntryId::from_index(0);
        let mut machine = FacetValue::empty();
        machine.machine = Some(MachineFacet {
            machine: item,
            config: Config::default(),
            count: None,
        });
        let result = FacetValue::scalar(2.0)
            .mul(&FacetValue::item(Monomial::new(
                3.0,
                [(BaseUnit::Item(item), 1)],
            )))
            .unwrap();
        assert_eq!(result.item.unwrap().coef(), 6.0);
        let result = FacetValue::scalar(2.0).mul(&machine).unwrap();
        assert_eq!(result.machine.unwrap().count, Some(2.0));
    }

    #[test]
    fn item_multiplication_drops_non_item_facets() {
        let item = EntryId::from_index(0);
        let mut a = FacetValue::item(Monomial::new(2.0, [(BaseUnit::Item(item), 1)]));
        a.config = Some(Config::default());
        let b = FacetValue::item(Monomial::new(3.0, [(BaseUnit::Item(item), 1)]));
        let result = a.mul(&b).unwrap();
        assert_eq!(result.item.unwrap().coef(), 6.0);
        assert!(result.config.is_none());
    }

    #[test]
    fn fractional_scaling_drops_config() {
        let module = EntryId::from_index(0);
        let mut value = FacetValue::item(Monomial::scalar(1.0));
        value.config = Some(Config {
            modules: vec![ModuleCount { module, count: 2 }],
            beacons: vec![],
        });
        let result = FacetValue::scalar(0.5).mul(&value).unwrap();
        assert!(result.config.is_none());
        assert!(result.item.is_some());
    }

    #[test]
    fn process_times_time_becomes_machine_seconds() {
        let mut value = FacetValue::empty();
        value.process = Some(Process {
            extent: Some(Extent::Machines(2.0)),
            net: BTreeMap::from([(BaseUnit::Joule, 4.0)]),
            power_w: Some(10.0),
            energy_j: None,
            recipes: BTreeSet::new(),
        });
        let time = FacetValue::item(Monomial::new(3.0, [(BaseUnit::Second, 1)]));
        let process = value.mul(&time).unwrap().process.unwrap();
        assert_eq!(process.extent, Some(Extent::MachineSeconds(6.0)));
        assert_eq!(process.net[&BaseUnit::Joule], 12.0);
        assert_eq!(process.energy_j, Some(30.0));
        assert_eq!(process.power_w, None);
    }

    #[test]
    fn machine_seconds_over_time_becomes_machines() {
        let mut value = FacetValue::empty();
        value.process = Some(Process {
            extent: Some(Extent::MachineSeconds(6.0)),
            net: BTreeMap::from([(BaseUnit::Joule, 12.0)]),
            power_w: None,
            energy_j: Some(30.0),
            recipes: BTreeSet::new(),
        });
        let time = FacetValue::item(Monomial::new(3.0, [(BaseUnit::Second, 1)]));
        let process = value.div(&time).unwrap().process.unwrap();
        assert_eq!(process.extent, Some(Extent::Machines(2.0)));
        assert_eq!(process.power_w, Some(10.0));
    }

    #[test]
    fn isolated_rate_times_time_becomes_capacity() {
        let mut value = FacetValue::empty();
        value.rate = Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(1), 15.0)])));
        let time = FacetValue::item(Monomial::new(2.0, [(BaseUnit::Second, 1)]));
        let capacity = value.mul(&time).unwrap().capacity.unwrap();
        assert_eq!(
            capacity,
            CapacityFacet::Items(BTreeMap::from([(Cap::Limit(1), 30.0)]))
        );
    }

    #[test]
    fn config_drops_under_division() {
        let module = EntryId::from_index(0);
        let mut value = FacetValue::item(Monomial::new(4.0, [(BaseUnit::Item(module), 1)]));
        value.config = Some(Config {
            modules: vec![ModuleCount { module, count: 2 }],
            beacons: vec![],
        });
        let result = value.div(&FacetValue::scalar(2.0)).unwrap();
        assert_eq!(result.item.unwrap().coef(), 2.0);
        assert!(result.config.is_none());

        let mut config_only = FacetValue::empty();
        config_only.config = Some(Config::default());
        assert_eq!(
            config_only.div(&FacetValue::scalar(2.0)),
            Err(FacetError::NoFacetSurvives)
        );
    }

    #[test]
    fn scalar_powers_use_raw_powf() {
        let item = EntryId::from_index(0);
        let value = FacetValue::scalar(4.0)
            .pow(&FacetValue::scalar(0.5))
            .unwrap();
        assert_eq!(value.item.unwrap().coef(), 2.0);
        let value = FacetValue::scalar(0.0)
            .pow(&FacetValue::scalar(0.0))
            .unwrap();
        assert_eq!(value.item.unwrap().coef(), 1.0);
        let value = FacetValue::scalar(-4.0)
            .pow(&FacetValue::scalar(0.5))
            .unwrap();
        assert!(value.item.unwrap().coef().is_nan());
        let unitful = FacetValue::item(Monomial::new(2.0, [(BaseUnit::Item(item), 1)]));
        assert_eq!(
            unitful.pow(&FacetValue::scalar(0.5)),
            Err(FacetError::FractionalExponent)
        );
    }

    #[test]
    fn config_juxtaposition_is_partial() {
        let module = EntryId::from_index(2);
        let beacon = EntryId::from_index(3);
        let cm = Config {
            modules: vec![ModuleCount { module, count: 2 }],
            beacons: vec![],
        };
        let cm2 = Config {
            modules: vec![ModuleCount { module, count: 3 }],
            beacons: vec![],
        };
        let ConfigJuxtaposition::Modules(merged) = cm.juxtapose(&cm2).unwrap() else {
            panic!("expected module merge");
        };
        assert_eq!(merged.modules[0].count, 5);

        let cb = Config {
            modules: vec![],
            beacons: vec![BeaconGroup {
                beacon,
                count: 1,
                modules: vec![],
            }],
        };
        let ConfigJuxtaposition::Attach(attached) = cb.juxtapose(&cm).unwrap() else {
            panic!("expected attach");
        };
        assert_eq!(attached.beacons[0].modules[0].count, 2);

        assert_eq!(cb.juxtapose(&cb), Err(FacetError::ConfigJuxtaposition));
        assert_eq!(cm.juxtapose(&cb), Err(FacetError::ConfigJuxtaposition));
    }

    #[test]
    fn config_addition_never_attaches_modules_to_beacons() {
        let module = EntryId::from_index(2);
        let beacon = EntryId::from_index(3);
        let mut a = FacetValue::empty();
        a.config = Some(Config {
            modules: vec![],
            beacons: vec![BeaconGroup {
                beacon,
                count: 1,
                modules: vec![],
            }],
        });
        let mut b = FacetValue::empty();
        b.config = Some(Config {
            modules: vec![ModuleCount { module, count: 4 }],
            beacons: vec![],
        });
        let config = a.add(&b).unwrap().config.unwrap();
        assert!(config.beacons[0].modules.is_empty());
        assert_eq!(config.modules[0].count, 4);
    }

    #[test]
    fn rate_and_capacity_application_use_item_stack_sizes() {
        let item = EntryId::from_index(0);
        let mut belt = FacetValue::empty();
        belt.rate = Some(RateFacet::Items(BTreeMap::from([
            (Cap::Full, 2.0),
            (Cap::Limit(4), 3.0),
        ])));
        let iron = FacetValue::item(Monomial::new(2.0, [(BaseUnit::Item(item), 1)]));
        let result = belt.juxtapose(&iron, Some(10)).unwrap();
        // 2 * min(Full,10) + 3 * min(Limit(4),10) = 20 + 12 = 32, times 2.
        assert_eq!(result.item.unwrap().coef(), 64.0);

        let mut chest = FacetValue::empty();
        chest.capacity = Some(CapacityFacet::Items(BTreeMap::from([(Cap::Full, 4.0)])));
        let result = chest.juxtapose(&iron, Some(10)).unwrap();
        assert_eq!(result.item.unwrap().coef(), 80.0);
    }

    #[test]
    fn juxtaposition_rejects_two_item_bearing_values() {
        let item = EntryId::from_index(0);
        let a = FacetValue::item(Monomial::new(1.0, [(BaseUnit::Item(item), 1)]));
        assert_eq!(
            a.juxtapose(&a, Some(10)),
            Err(FacetError::UnsupportedOperation)
        );
    }

    #[test]
    fn isolated_rate_division_and_conversion() {
        let mut rate = FacetValue::empty();
        rate.rate = Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(1), 10.0)])));
        let mut rate2 = FacetValue::empty();
        rate2.rate = Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(1), 2.0)])));
        let result = rate.div(&rate2).unwrap();
        assert_eq!(result.item.unwrap().coef(), 5.0);

        let mut capacity = FacetValue::empty();
        capacity.capacity = Some(CapacityFacet::Items(BTreeMap::from([(
            Cap::Limit(1),
            30.0,
        )])));
        let result = rate.div(&capacity).unwrap();
        assert_eq!(result.item.unwrap().units()[&BaseUnit::Second], -1);
        let result = capacity.div(&rate).unwrap();
        assert_eq!(result.item.unwrap().units()[&BaseUnit::Second], 1);
    }

    #[test]
    fn multi_term_rate_division_errors() {
        let mut a = FacetValue::empty();
        a.rate = Some(RateFacet::Items(BTreeMap::from([
            (Cap::Limit(1), 1.0),
            (Cap::Limit(2), 2.0),
        ])));
        let mut b = FacetValue::empty();
        b.rate = Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(1), 1.0)])));
        assert_eq!(a.div(&b), Err(FacetError::MultiTermRate));
    }

    #[test]
    fn juxtaposition_handles_partial_config_merge() {
        let module = EntryId::from_index(2);
        let beacon = EntryId::from_index(3);
        let mut cm = FacetValue::empty();
        cm.config = Some(Config {
            modules: vec![ModuleCount { module, count: 2 }],
            beacons: vec![],
        });
        let mut cb = FacetValue::item(Monomial::scalar(1.0));
        cb.config = Some(Config {
            modules: vec![],
            beacons: vec![BeaconGroup {
                beacon,
                count: 1,
                modules: vec![],
            }],
        });
        let attached = cb.juxtapose(&cm, None).unwrap();
        assert_eq!(attached.config.unwrap().beacons[0].modules[0].count, 2);
        assert!(attached.item.is_some());

        let modules = cm.juxtapose(&cm, None).unwrap();
        assert!(modules.item.is_none());
        assert_eq!(modules.config.unwrap().modules[0].count, 4);

        assert_eq!(
            cm.juxtapose(&cb, None),
            Err(FacetError::ConfigJuxtaposition)
        );
    }

    #[test]
    fn config_canonicalization_sorts_and_elides_zero() {
        let a = EntryId::from_index(1);
        let b = EntryId::from_index(0);
        let mut config = Config {
            modules: vec![
                ModuleCount {
                    module: a,
                    count: 2,
                },
                ModuleCount {
                    module: b,
                    count: 1,
                },
                ModuleCount {
                    module: a,
                    count: 0,
                },
            ],
            beacons: vec![],
        };
        config.canonicalize();
        assert_eq!(
            config.modules,
            vec![
                ModuleCount {
                    module: b,
                    count: 1
                },
                ModuleCount {
                    module: a,
                    count: 2
                }
            ]
        );
    }
}
