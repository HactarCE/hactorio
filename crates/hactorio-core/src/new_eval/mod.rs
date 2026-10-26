use std::collections::BTreeMap;

use hactorio_data::{EntryId, Registry};
use thiserror::Error;

mod config;
mod process;

use config::*;
use process::*;

type Result<T> = std::result::Result<T, ValueError>;

macro_rules! impl_enum_from {
    (impl From<$ty1:ident> for $ty2:ident) => {
        impl_enum_from!(impl From<$ty1> for <$ty2>::$ty1);
    };
    (impl From<$ty1:ty> for <$ty2:ty>::$variant:ident) => {
        impl From<$ty1> for $ty2{
            fn from(value: $ty1) -> Self {
                Self::$variant(value)
            }
        }
    };
}

#[derive(Error, Debug)]
pub enum ValueError {
    #[error("expected module; got {0:?}")]
    ExpectedModule(Box<str>),
    #[error("expected item; got {0:?}")]
    ExpectedItem(Box<str>),
    #[error("expected fluid; got {0:?}")]
    ExpectedFluid(Box<str>),
    #[error("expected item or fluid; got {0:?}")]
    ExpectedItemOrFluid(Box<str>),
    #[error("expected small integer; got {0:?}")]
    ExpectedSmallInteger(f64),
    #[error("overflow")]
    Overflow,
    #[error("unsupported operation")]
    UnsupportedOperation,
    #[error("multivalue loses all components")]
    LostAllComponents,
}

#[derive(Debug, Clone)]
pub enum Value {
    MultiValue(MultiValue),
    MachineMakingRecipe(MachineMakingRecipe),
    Process(Process),
    Quality(Quality),
}

impl_enum_from!(impl From<MultiValue> for Value);
impl_enum_from!(impl From<MachineMakingRecipe> for Value);
impl_enum_from!(impl From<Process> for Value);
impl_enum_from!(impl From<Quality> for Value);

impl Value {
    pub fn as_multi_value(&self) -> Option<&MultiValue> {
        match self {
            Value::MultiValue(m) => Some(m),
            _ => None,
        }
    }

    pub fn to_dimensionless_number(&self) -> Option<f64> {
        self.as_multi_value()?.to_dimensionless_number()
    }

    /// Juxtaposition (operands placed directly next to each other with no
    /// operator)
    pub fn juxtapose(&self, other: &Self) -> Result<Self> {
        if let Some(scalar) = other.to_dimensionless_number() {
            // value * scalar
            self.scale(scalar)
        } else if let Some(scalar) = self.to_dimensionless_number() {
            // scalar * value
            other.scale(scalar)
        } else {
            todo!("probably more combinations I haven't thought of")
        }
    }

    /// `+` operator
    pub fn add(&self, other: &Self) -> Result<Self> {
        todo!()
    }

    /// `-` operator
    pub fn sub(&self, other: &Self) -> Result<Self> {
        todo!()
    }

    /// `*` operator
    pub fn mul(&self, other: &Self) -> Result<Self> {
        todo!()
    }

    /// `/` operator
    pub fn div(&self, other: &Self) -> Result<Self> {
        todo!()
    }

    /// `into` operator
    pub fn scale_by_input(&self, other: &Self) -> Result<Self> {
        todo!()
    }

    /// `from` operator
    pub fn scale_by_output(&self, other: &Self) -> Result<Self> {
        todo!()
    }

    /// `in` operator
    pub fn scale_by_net(&self, other: &Self) -> Result<Self> {
        todo!()
    }

    /// Comparison operators
    pub fn filter_cmp(&self, comparator: Comparator, other: &Self) -> Result<Self> {
        todo!()
    }

    pub fn scale(&self, scalar: f64) -> Result<Self> {
        match self {
            Value::MultiValue(multi_value) => Ok(multi_value.scale(scalar)?.into()),
            Value::MachineMakingRecipe(_) => Err(ValueError::UnsupportedOperation),
            Value::Process(process) => Ok(process.scale(scalar).into()),
            Value::Quality(_) => Err(ValueError::UnsupportedOperation),
        }
    }
}

pub enum Comparator {
    Lt,
    Gt,
    Lte,
    Gte,
    Eq,
    Ne,
}
impl Comparator {
    pub fn eval<T: Ord>(self, lhs: T, rhs: T) -> bool {
        match self {
            Comparator::Lt => lhs < rhs,
            Comparator::Gt => lhs > rhs,
            Comparator::Lte => lhs <= rhs,
            Comparator::Gte => lhs >= rhs,
            Comparator::Eq => lhs == rhs,
            Comparator::Ne => lhs != rhs,
        }
    }
}

/// Set of values of different types.
///
/// Given:
///
/// - `Q` = quantity (`S` if dimensionless scalar)
/// - `T` = stack rate
/// - `K` = stack capacity
/// - `C` = config
/// - `I` = item
/// - `F` = fluid
/// - `R` = recipe
///
/// The following operations are supported:
///
/// - `Q` supports:
///     - Addition/subtraction/multiplication/division with itself
///     - Juxtaposition with `S` on either side.
/// - `T` and `K` support:
///     - Addition with themselves
///     - Multiplication/juxtaposition by `S` on either side
///     - Juxtaposition with an item or fluid on the right
/// - `C` supports:
///     - Addition and juxtaposition with itself
///     - Juxtaposition with a beacon item (`I`) on the left
///     - `with` operator with a machine item (`I`) or [`MachineMakingRecipe`]
///       on the left (results in [`Process`])
/// - `I` and `F` support juxtaposition with `T` or `K` on the left (results in
///   `Q`)
#[derive(Debug, Clone)]
struct MultiValue {
    /// General quantity of items, time, energy, or some combination thereof.
    ///
    /// May overlap with any other facet.
    quantity: Option<Quantity>,

    /// Number of (possibly limited) stacks that entity can move per second.
    ///
    /// May overlap with `quantity`, `stack_capacity`, `item`, and `recipe`.
    stack_rate: Option<Stacks>,
    /// Number of (possibly limited) stacks that fit in this entity.
    ///
    /// May overlap with `quantity`, `stack_rate`, `item`, and `recipe`.
    stack_capacity: Option<Stacks>,

    /// Machine module/beacon configuration.
    ///
    /// May overlap with `quantity`, `item`, and `recipe`.
    config: Option<Config>,

    /// Unscaled item.
    ///
    /// May overlap with any other facet except `fluid`. Erased on any
    /// operation.
    item: Option<EntryId>,
    /// Unscaled fluid.
    ///
    /// May overlap with any other facet except `item`. Erased on any operation.
    fluid: Option<EntryId>,
    /// Unscaled recipe.
    ///
    /// May overlap with any other facet. Erased on any operation.
    recipe: Option<EntryId>,
}

impl MultiValue {
    fn if_nonempty(self) -> Result<Self> {
        let Self {
            quantity,
            stack_rate,
            stack_capacity,
            config,
            item,
            fluid,
            recipe,
        } = &self;
        if quantity.is_some()
            || stack_rate.is_some()
            || stack_capacity.is_some()
            || config.is_some()
            || item.is_some()
            || fluid.is_some()
            || recipe.is_some()
        {
            Ok(self)
        } else {
            Err(ValueError::LostAllComponents)
        }
    }

    pub fn to_dimensionless_number(&self) -> Option<f64> {
        self.quantity.as_ref()?.to_number()
    }

    pub fn scale(&self, scalar: f64) -> Result<Self> {
        Self {
            quantity: self.quantity.as_ref().map(|q| q.scale(scalar)),
            stack_rate: self.stack_rate.as_ref().map(|s| s.scale(scalar)),
            stack_capacity: self.stack_capacity.as_ref().map(|c| c.scale(scalar)),
            config: self.config.as_ref().map(|c| c.scale(scalar)).transpose()?,
            item: None,
            fluid: None,
            recipe: None,
        }
        .if_nonempty()
    }
}

#[derive(Debug, Clone)]
enum Stacks {
    Items(ItemStacks),
    Fluid(f64),
}

impl_enum_from!(impl From<ItemStacks> for <Stacks>::Items);

impl Stacks {
    pub fn of_resource(&self, resource: Resource, registry: &Registry) -> Result<Quantity> {
        match (self, resource) {
            (Stacks::Items(item_stacks), Resource::Item(item)) => Ok(
                Quantity::from_units_of_resource(item.into(), item_stacks.of(item, registry)?),
            ),
            (Stacks::Items(_), Resource::Fluid(f)) => {
                Err(ValueError::ExpectedItem(registry[f.id].id.clone()))
            }
            (Stacks::Fluid(_), Resource::Item(i)) => {
                Err(ValueError::ExpectedFluid(registry[i.id].id.clone()))
            }
            (Stacks::Fluid(fluid_scalar), Resource::Fluid(fluid)) => Ok(
                Quantity::from_units_of_resource(fluid.into(), *fluid_scalar),
            ),
        }
    }

    pub fn scale(&self, scalar: f64) -> Self {
        match self {
            Stacks::Items(item_stacks) => item_stacks.scale(scalar).into(),
            Stacks::Fluid(f) => Self::Fluid(*f * scalar),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemStacks(BTreeMap<StackLimit, f64>);

impl ItemStacks {
    pub fn of(&self, item: Item, registry: &Registry) -> Result<f64> {
        self.0
            .iter()
            .map(|(limit, multiplier)| Ok(limit.stack_size(item, registry)? as f64 * multiplier))
            .sum()
    }

    pub fn scale(&self, scalar: f64) -> Self {
        Self(self.0.iter().map(|(k, v)| (*k, *v * scalar)).collect())
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StackLimit {
    Limit(u32),
    #[default]
    Full,
}

impl StackLimit {
    pub fn stack_size(self, item: Item, registry: &Registry) -> Result<u32> {
        let entry = registry.entry_by_index(item.id);
        let Some(item) = &entry.item else {
            return Err(ValueError::ExpectedItem(entry.id.clone()));
        };
        match self {
            StackLimit::Limit(limit) => Ok(std::cmp::min(limit, item.stack_size)),
            StackLimit::Full => Ok(item.stack_size),
        }
    }
}

/// A quantity value: either a plain number or one coefficient per resource,
/// optionally divided by one resource, times `second^s * joule^j * craft^c` for
/// integers _s_, _j_, _c_.
///
/// The scalar and resource cases are exclusive. `3 + 2 iron` is not
/// representable. Because each resource is its own dimension, several
/// coefficients describe a bundle of distinct quantities rather than one
/// magnitude.
#[derive(Debug, Clone)]
pub struct Quantity {
    /// Plain number or coefficient per resource.
    amount: Amount,
    /// Resource in the denominator, if any.
    per: Option<Resource>,
    /// Exponent for `second` base unit.
    second: i32,
    /// Exponent for `joule` base unit.
    joule: i32,
    /// Exponent for `craft` base unit.
    craft: i32,
}

impl Quantity {
    pub fn to_number(&self) -> Option<f64> {
        self.amount.to_number()
    }

    pub fn scale(&self, scalar: f64) -> Self {
        Self {
            amount: self.amount.scale(scalar),
            per: self.per,
            second: self.second,
            joule: self.joule,
            craft: self.craft,
        }
    }

    pub fn from_units_of_resource(resource: Resource, units: f64) -> Self {
        Self {
            amount: Amount::from_units_of_resource(resource, units),
            per: None,
            second: 0,
            joule: 0,
            craft: 0,
        }
    }
}

/// Plain number or coefficient per resource.
#[derive(Debug, Clone)]
pub enum Amount {
    /// Plain number (dimensionless scalar).
    Number(f64),
    /// Coefficient per resource.
    ///
    /// Zero terms are removed from the map.
    Resources(BTreeMap<Resource, f64>),
}

impl Default for Amount {
    fn default() -> Self {
        Self::Number(0.0)
    }
}

impl_enum_from!(impl From<f64> for <Amount>::Number);

impl Amount {
    pub fn to_number(&self) -> Option<f64> {
        match self {
            Amount::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn scale(&self, scalar: f64) -> Self {
        match self {
            Amount::Number(n) => Amount::Number(n * scalar),
            Amount::Resources(map) => {
                Amount::Resources(map.iter().map(|(k, v)| (k.clone(), *v * scalar)).collect())
            }
        }
    }

    pub fn from_units_of_resource(resource: Resource, units: f64) -> Self {
        Self::Resources(BTreeMap::from_iter([(resource, units)])).canonicalize()
    }

    pub fn canonicalize(mut self) -> Self {
        if let Self::Resources(resource_list) = &mut self {
            resource_list.retain(|_k, v| crate::approx::is_nonzero(*v));
        }
        self
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Resource {
    Item(Item),
    Fluid(Fluid),
}

impl_enum_from!(impl From<Item> for Resource);
impl_enum_from!(impl From<Fluid> for Resource);

impl Resource {
    pub fn from_entry(entry_id: EntryId, registry: &Registry) -> Result<Self> {
        let entry = &registry[entry_id];
        if entry.is_item_or_fluid() {
            Ok(if entry.item.is_some() {
                Self::Item(Item {
                    id: entry_id,
                    quality: Quality::Common,
                })
            } else {
                Self::Fluid(Fluid { id: entry_id })
            })
        } else {
            Err(ValueError::ExpectedItemOrFluid(entry.id.clone()))
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Item {
    id: EntryId,
    quality: Quality,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Quality {
    #[default]
    Common = 1,
    Uncommon = 2,
    Rare = 3,
    Epic = 4,
    Legendary = 5,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fluid {
    pub id: EntryId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Recipe {
    pub id: EntryId,
    pub quality: Option<Quality>,
}
