//! Evaluation of the facet syntax tree.

use std::collections::{BTreeMap, HashMap};

use hactorio_data::{EntryId, Registry};
use thiserror::Error;

use crate::facet_machine;
use crate::facet_parser::{Expr, ExprKind, Property, Selector, Signal, Statement, StatementKind};
use crate::facet_token::token_value;
use crate::facets::{
    Cap, CapacityFacet, Extent, FacetError, FacetValue, Monomial, Process, RateFacet,
};
use crate::settings::Settings;
use crate::syntax::Span;
use crate::value::BaseUnit;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct EvalError {
    pub span: Span,
    pub message: String,
}

impl EvalError {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        EvalError {
            span,
            message: message.into(),
        }
    }
}

pub type Variables = HashMap<String, Result<FacetValue, EvalError>>;

pub fn eval_expr(
    expr: &Expr,
    variables: &Variables,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let span = expr.span;
    match &expr.kind {
        ExprKind::Number(value) => Ok(FacetValue::scalar(*value)),
        ExprKind::Token(id) => token(*id, span, registry, settings),
        ExprKind::Quantity { count, token: id } => {
            let value = token(*id, span, registry, settings)?;
            scale_literal(value, *count, span)
        }
        ExprKind::Time { count, unit } => Ok(FacetValue::item(Monomial::new(
            count * unit.seconds(),
            [(BaseUnit::Second, 1)],
        ))),
        ExprKind::Joule(value) => Ok(FacetValue::item(Monomial::new(
            *value,
            [(BaseUnit::Joule, 1)],
        ))),
        ExprKind::Watt(value) => Ok(FacetValue::item(Monomial::new(
            *value,
            [(BaseUnit::Joule, 1), (BaseUnit::Second, -1)],
        ))),
        ExprKind::Craft(value) => Ok(FacetValue::item(Monomial::new(
            *value,
            [(BaseUnit::Craft, 1)],
        ))),
        ExprKind::Variable(name) => match variables.get(name) {
            Some(Ok(value)) => Ok(value.clone()),
            Some(Err(error)) => Err(error.clone()),
            None => Err(EvalError::new(span, format!("unknown variable `{name}`"))),
        },
        ExprKind::Neg(inner) => {
            let value = eval_expr(inner, variables, registry, settings)?;
            map_facet(value.neg(), span)
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let lhs = eval_expr(lhs, variables, registry, settings)?;
            let rhs = eval_expr(rhs, variables, registry, settings)?;
            let result = match op {
                crate::syntax::BinaryOp::Add => lhs.add(&rhs),
                crate::syntax::BinaryOp::Sub => lhs.sub(&rhs),
                crate::syntax::BinaryOp::Mul => lhs.mul(&rhs),
                crate::syntax::BinaryOp::Div => lhs.div(&rhs),
                crate::syntax::BinaryOp::Pow => lhs.pow(&rhs),
            };
            map_facet(result, span)
        }
        ExprKind::Juxtapose { lhs, rhs } => {
            let lhs = eval_expr(lhs, variables, registry, settings)?;
            let rhs = eval_expr(rhs, variables, registry, settings)?;
            let stack = item_stack_size(&rhs, registry);
            map_facet(lhs.juxtapose(&rhs, stack), span)
        }
        ExprKind::With { machine, config } => {
            let machine = eval_expr(machine, variables, registry, settings)?;
            let config = eval_expr(config, variables, registry, settings)?;
            with(machine, config, span, registry)
        }
        ExprKind::Making { machine, recipe } => {
            let machine = eval_expr(machine, variables, registry, settings)?;
            let recipe = eval_expr(recipe, variables, registry, settings)?;
            making(machine, recipe, span, registry, settings)
        }
        ExprKind::In { quantity, machine } => {
            let quantity = eval_expr(quantity, variables, registry, settings)?;
            let machine = eval_expr(machine, variables, registry, settings)?;
            in_expression(quantity, machine, span, registry, settings)
        }
        ExprKind::StackSize(inner) => {
            let value = eval_expr(inner, variables, registry, settings)?;
            stack_size(value, span, registry, settings)
        }
        ExprKind::Property { value, selector } => {
            let value = eval_expr(value, variables, registry, settings)?;
            property(value, selector, span, registry, settings)
        }
    }
}

pub fn eval_statement(
    statement: &Statement,
    variables: &mut Variables,
    registry: &Registry,
    settings: &Settings,
) -> Option<Result<FacetValue, EvalError>> {
    match &statement.kind {
        StatementKind::Blank | StatementKind::Comment => None,
        StatementKind::Expression(expr) => Some(eval_expr(expr, variables, registry, settings)),
        StatementKind::Definition { name, value } => {
            if variables.contains_key(name) {
                return Some(Err(EvalError::new(
                    statement.span,
                    format!("`{name}` is already defined"),
                )));
            }
            let result = eval_expr(value, variables, registry, settings);
            variables.insert(name.clone(), result.clone());
            Some(result)
        }
    }
}

fn token(
    id: EntryId,
    span: Span,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    token_value(id, registry, settings).map_err(|error| EvalError::new(span, error.to_string()))
}

fn scale_literal(value: FacetValue, count: f64, span: Span) -> Result<FacetValue, EvalError> {
    map_facet(value.mul(&FacetValue::scalar(count)), span)
}

fn map_facet(result: Result<FacetValue, FacetError>, span: Span) -> Result<FacetValue, EvalError> {
    result.map_err(|error| EvalError::new(span, error.to_string()))
}

fn item_stack_size(value: &FacetValue, registry: &Registry) -> Option<u32> {
    let item = value.item.as_ref()?;
    if item.units().len() != 1 || item.units().values().next() != Some(&1) {
        return None;
    }
    let (unit, _) = item.units().iter().next()?;
    let BaseUnit::Item(id) = unit else {
        return None;
    };
    registry
        .entry_by_index(*id)
        .item
        .as_ref()
        .map(|item| item.stack_size)
}

fn with(
    machine: FacetValue,
    config: FacetValue,
    span: Span,
    registry: &Registry,
) -> Result<FacetValue, EvalError> {
    let mut machine = machine
        .machine
        .ok_or_else(|| EvalError::new(span, "`with` needs a machine on the left"))?;
    let config = config
        .config
        .ok_or_else(|| EvalError::new(span, "`with` needs a config on the right"))?;
    machine
        .config
        .merge_add(&config)
        .map_err(|error| EvalError::new(span, error.to_string()))?;
    facet_machine::validate(registry, machine.machine, &machine.config, None)
        .map_err(|error| EvalError::new(span, error))?;
    Ok(FacetValue {
        machine: Some(machine),
        ..FacetValue::empty()
    })
}

fn making(
    machine: FacetValue,
    recipe: FacetValue,
    span: Span,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let machine = machine
        .machine
        .ok_or_else(|| EvalError::new(span, "`making` needs a machine on the left"))?;
    let recipe = recipe
        .recipe
        .ok_or_else(|| EvalError::new(span, "`making` needs a recipe on the right"))?;
    let recipe_data = registry.recipe(recipe);
    facet_machine::validate(
        registry,
        machine.machine,
        &machine.config,
        Some(recipe_data),
    )
    .map_err(|error| EvalError::new(span, error))?;
    let net = facet_machine::coefficients(
        registry,
        machine.machine,
        &machine.config,
        recipe_data,
        settings,
    )
    .map_err(|error| EvalError::new(span, error))?;
    let modifiers = facet_machine::modifiers(
        registry,
        machine.machine,
        &machine.config,
        Some(recipe_data),
        settings,
    );
    let (extent, net, power_w) = match machine.count {
        None => (None, net, Some(modifiers.power_w)),
        Some(count) => (
            Some(Extent::Machines(count)),
            net.into_iter()
                .map(|(unit, amount)| (unit, amount * count))
                .collect(),
            Some(modifiers.power_w * count),
        ),
    };
    Ok(FacetValue {
        process: Some(Process {
            extent,
            net,
            power_w,
            energy_j: None,
            recipes: [recipe].into_iter().collect(),
        }),
        ..FacetValue::empty()
    })
}

struct InQuantity {
    unit: BaseUnit,
    coefficient: f64,
    rate: bool,
    craft: bool,
}

fn in_quantity(value: &FacetValue) -> Option<InQuantity> {
    let item = value.item.as_ref()?;
    let mut units = item.units().iter();
    let (unit, exponent) = units.next()?;
    if *exponent != 1 {
        return None;
    }
    let mut second = false;
    if let Some((next, next_exponent)) = units.next() {
        if *next == BaseUnit::Second && *next_exponent == -1 && units.next().is_none() {
            second = true;
        } else {
            return None;
        }
    }
    let craft = *unit == BaseUnit::Craft;
    if !matches!(
        unit,
        BaseUnit::Item(_) | BaseUnit::Fluid(_) | BaseUnit::Craft
    ) {
        return None;
    }
    Some(InQuantity {
        unit: *unit,
        coefficient: item.coef(),
        rate: second,
        craft,
    })
}

fn in_expression(
    quantity: FacetValue,
    machine: FacetValue,
    span: Span,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let quantity = in_quantity(&quantity)
        .ok_or_else(|| EvalError::new(span, "`in` needs a pure item/fluid/craft quantity"))?;
    let (recipe, net, power_w) = if let Some(process) = &machine.process {
        if process.extent.is_some() {
            return Err(EvalError::new(span, "`in` cannot use a solved process"));
        }
        if process.recipes.len() != 1 {
            return Err(EvalError::new(
                span,
                "`in` needs an unsolved process with exactly one recipe",
            ));
        }
        let recipe = *process.recipes.iter().next().unwrap();
        let net = process.net.clone();
        let power = process.power_w.unwrap_or(0.0);
        (recipe, net, power)
    } else if let Some(machine_facet) = &machine.machine {
        if machine_facet.count.is_some() {
            return Err(EvalError::new(
                span,
                "`in` cannot use a machine with an attached count",
            ));
        }
        if quantity.craft {
            return Err(EvalError::new(
                span,
                "craft quantities need an explicit recipe",
            ));
        }
        let recipe = facet_machine::resolve_recipe(
            registry,
            machine_facet.machine,
            &machine_facet.config,
            quantity.unit,
        )
        .map_err(|error| EvalError::new(span, error))?;
        let recipe_data = registry.recipe(recipe);
        facet_machine::validate(
            registry,
            machine_facet.machine,
            &machine_facet.config,
            Some(recipe_data),
        )
        .map_err(|error| EvalError::new(span, error))?;
        let net = facet_machine::coefficients(
            registry,
            machine_facet.machine,
            &machine_facet.config,
            recipe_data,
            settings,
        )
        .map_err(|error| EvalError::new(span, error))?;
        let modifiers = facet_machine::modifiers(
            registry,
            machine_facet.machine,
            &machine_facet.config,
            Some(recipe_data),
            settings,
        );
        (recipe, net, modifiers.power_w)
    } else {
        return Err(EvalError::new(
            span,
            "`in` needs a machine or process on the right",
        ));
    };

    let denominator = if quantity.craft {
        net.get(&BaseUnit::Craft).copied().unwrap_or(0.0)
    } else {
        net.get(&quantity.unit).copied().unwrap_or(0.0)
    };
    if denominator == 0.0 {
        return Err(EvalError::new(
            span,
            "selected unit has zero net coefficient",
        ));
    }
    let magnitude = if quantity.craft {
        quantity.coefficient / denominator
    } else {
        quantity.coefficient / denominator.abs()
    };
    let (extent, net, energy_j, power_w) = if quantity.rate {
        (
            Some(Extent::Machines(magnitude)),
            net.into_iter()
                .map(|(unit, amount)| (unit, amount * magnitude))
                .collect(),
            None,
            Some(power_w * magnitude),
        )
    } else {
        (
            Some(Extent::MachineSeconds(magnitude)),
            net.into_iter()
                .map(|(unit, amount)| (unit, amount * magnitude))
                .collect(),
            Some(power_w * magnitude),
            None,
        )
    };
    Ok(FacetValue {
        process: Some(Process {
            extent,
            net,
            power_w,
            energy_j,
            recipes: [recipe].into_iter().collect(),
        }),
        ..FacetValue::empty()
    })
}

fn stack_size(
    value: FacetValue,
    span: Span,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let level = settings.belt_stacking.clamp(1, 4);
    let cap = Cap::Limit(u32::from(level));
    if let Some(belt) = value.belt {
        let item = match &value.item {
            Some(item) => {
                let (unit, exponent) = item
                    .units()
                    .iter()
                    .next()
                    .ok_or_else(|| EvalError::new(span, "belt item has no unit"))?;
                if *exponent != 1 || item.units().len() != 1 {
                    return Err(EvalError::new(span, "belt item must be a pure item"));
                }
                let BaseUnit::Item(id) = unit else {
                    return Err(EvalError::new(span, "belt item must be an item"));
                };
                let stack = registry
                    .entry_by_index(*id)
                    .item
                    .as_ref()
                    .map(|item| item.stack_size)
                    .ok_or_else(|| EvalError::new(span, "belt item has no stack size"))?;
                Some(Monomial::new(
                    item.coef() * f64::from(stack),
                    [(BaseUnit::Item(*id), 1)],
                ))
            }
            None => None,
        };
        let rate = value.rate.map(|rate| cap_rate(rate, cap));
        return Ok(FacetValue {
            item,
            recipe: value.recipe,
            belt: Some(crate::facets::BeltFacet {
                count: belt.count,
                cap,
            }),
            rate,
            ..FacetValue::empty()
        });
    }

    let item = value
        .item
        .ok_or_else(|| EvalError::new(span, "`:stack-size:` needs an item or belt"))?;
    let (unit, exponent) = item
        .units()
        .iter()
        .next()
        .ok_or_else(|| EvalError::new(span, "`:stack-size:` needs an item unit"))?;
    if *exponent != 1 || item.units().len() != 1 {
        return Err(EvalError::new(span, "`:stack-size:` needs a pure item"));
    }
    let BaseUnit::Item(id) = unit else {
        return Err(EvalError::new(span, "`:stack-size:` does not stack fluids"));
    };
    let stack = registry
        .entry_by_index(*id)
        .item
        .as_ref()
        .map(|item| item.stack_size)
        .ok_or_else(|| EvalError::new(span, "item has no stack size"))?;
    Ok(FacetValue::item(Monomial::new(
        item.coef() * f64::from(stack),
        [(BaseUnit::Item(*id), 1)],
    )))
}

fn cap_rate(rate: RateFacet, cap: Cap) -> RateFacet {
    match rate {
        RateFacet::Items(items) => RateFacet::Items(
            [(cap, items.values().copied().sum::<f64>())]
                .into_iter()
                .filter(|(_, amount)| *amount != 0.0)
                .collect(),
        ),
        RateFacet::Fluid(value) => RateFacet::Fluid(value),
    }
}

fn property(
    value: FacetValue,
    selector: &Selector,
    span: Span,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let property = match selector {
        Selector::Property(property) => *property,
        Selector::Signal(Signal::Clock) => Property::Time,
        Selector::Signal(Signal::Speed) => Property::Speed,
        Selector::Signal(Signal::Lightning) => Property::Electric,
        Selector::Signal(Signal::StackSize) => Property::StackSize,
        Selector::Token(_) => {
            return unit_selector(value, selector, span, registry);
        }
    };
    match property {
        Property::Rate => value
            .rate
            .map(|rate| FacetValue {
                rate: Some(rate),
                ..FacetValue::empty()
            })
            .ok_or_else(|| EvalError::new(span, "`.rate` needs a rate facet")),
        Property::Capacity => capacity_property(value, span),
        Property::StackSize => {
            let item = value
                .item
                .as_ref()
                .ok_or_else(|| EvalError::new(span, "`.stack-size` needs an item"))?;
            let (unit, exponent) = item
                .units()
                .iter()
                .next()
                .ok_or_else(|| EvalError::new(span, "`.stack-size` needs an item"))?;
            if *exponent != 1 || item.units().len() != 1 {
                return Err(EvalError::new(span, "`.stack-size` needs a pure item"));
            }
            let BaseUnit::Item(id) = unit else {
                return Err(EvalError::new(span, "`.stack-size` needs an item"));
            };
            let stack = registry
                .entry_by_index(*id)
                .item
                .as_ref()
                .map(|item| item.stack_size)
                .ok_or_else(|| EvalError::new(span, "item has no stack size"))?;
            Ok(FacetValue::scalar(f64::from(stack)))
        }
        Property::Power => power_property(&value, span, registry, settings, false),
        Property::Electric => power_property(&value, span, registry, settings, true),
        Property::Energy => energy_property(&value, span, registry),
        Property::Machines => machines_property(&value, span),
        Property::Time => time_property(&value, span),
        Property::Crafts => crafts_property(&value, span),
        Property::Speed
        | Property::Productivity
        | Property::Prod
        | Property::Pollution
        | Property::Slots => machine_scalar(&value, property, span, registry, settings),
        Property::Recipe => value
            .process
            .as_ref()
            .filter(|process| process.recipes.len() == 1)
            .map(|process| FacetValue {
                recipe: Some(*process.recipes.iter().next().unwrap()),
                ..FacetValue::empty()
            })
            .ok_or_else(|| EvalError::new(span, "`.recipe` needs a process with one recipe")),
        Property::Abs => value
            .abs()
            .map_err(|error| EvalError::new(span, error.to_string())),
    }
}

fn capacity_property(value: FacetValue, span: Span) -> Result<FacetValue, EvalError> {
    if let Some(capacity) = value.capacity {
        return Ok(FacetValue {
            capacity: Some(capacity),
            ..FacetValue::empty()
        });
    }
    let belt = value
        .belt
        .ok_or_else(|| EvalError::new(span, "`.capacity` needs a capacity or belt"))?;
    Ok(FacetValue {
        capacity: Some(CapacityFacet::Items(BTreeMap::from([(
            belt.cap,
            belt.count * 8.0,
        )]))),
        ..FacetValue::empty()
    })
}

fn power_property(
    value: &FacetValue,
    span: Span,
    registry: &Registry,
    settings: &Settings,
    electric: bool,
) -> Result<FacetValue, EvalError> {
    if let Some(machine) = &value.machine {
        let modifiers =
            facet_machine::modifiers(registry, machine.machine, &machine.config, None, settings);
        let count = machine.count.unwrap_or(1.0);
        return Ok(watts(modifiers.power_w * count));
    }
    if let Some(process) = &value.process {
        if let Some(power) = process.power_w {
            return Ok(watts(power));
        }
        if let Some(energy) = process.energy_j {
            if electric {
                return Ok(joules(energy));
            }
            return Err(EvalError::new(
                span,
                "`.power` is not defined for machine-seconds",
            ));
        }
        return Err(EvalError::new(span, "process has no power or energy"));
    }
    if let Some(config) = &value.config {
        if !config.beacons.is_empty() {
            let power: f64 = config
                .beacons
                .iter()
                .map(|group| {
                    f64::from(group.count)
                        * registry
                            .entry_by_index(group.beacon)
                            .beacon
                            .as_ref()
                            .map_or(0.0, |beacon| beacon.energy_usage_w)
                })
                .sum();
            return Ok(watts(power));
        }
    }
    if let Some(item) = &value.item {
        let (unit, exponent) = item
            .units()
            .iter()
            .next()
            .ok_or_else(|| EvalError::new(span, "need an item"))?;
        if *exponent != 1 || item.units().len() != 1 {
            return Err(EvalError::new(span, "need a pure item"));
        }
        let BaseUnit::Item(id) = unit else {
            return Err(EvalError::new(span, "need an item"));
        };
        let entry = registry.entry_by_index(*id);
        let equipment = entry.equipment;
        let power = entry
            .item
            .as_ref()
            .and_then(|item| item.power_w)
            .or_else(|| equipment.and_then(|equipment| equipment.power_w));
        let stored = entry
            .item
            .as_ref()
            .and_then(|item| item.stored_energy_j)
            .or_else(|| equipment.and_then(|equipment| equipment.stored_energy_j));
        if electric && power.is_some() && stored.is_some() {
            return Err(EvalError::new(
                span,
                "`.electric` is ambiguous: the item has both power and stored energy",
            ));
        }
        if let Some(power) = power {
            return Ok(watts(power * item.coef()));
        }
        if let Some(energy) = stored {
            if electric {
                return Ok(joules(energy * item.coef()));
            }
        }
    }
    Err(EvalError::new(span, "no power for this value"))
}

fn energy_property(
    value: &FacetValue,
    span: Span,
    registry: &Registry,
) -> Result<FacetValue, EvalError> {
    if let Some(process) = &value.process {
        return process
            .energy_j
            .map(joules)
            .ok_or_else(|| EvalError::new(span, "process has no energy"));
    }
    if let Some(item) = &value.item {
        let (unit, exponent) = item
            .units()
            .iter()
            .next()
            .ok_or_else(|| EvalError::new(span, "need an item"))?;
        if *exponent != 1 || item.units().len() != 1 {
            return Err(EvalError::new(span, "need a pure item"));
        }
        let BaseUnit::Item(id) = unit else {
            return Err(EvalError::new(span, "need an item"));
        };
        let entry = registry.entry_by_index(*id);
        let stored = entry
            .item
            .as_ref()
            .and_then(|item| item.stored_energy_j)
            .or_else(|| entry.equipment.and_then(|e| e.stored_energy_j));
        if let Some(energy) = stored {
            return Ok(joules(energy * item.coef()));
        }
    }
    Err(EvalError::new(span, "no stored energy for this value"))
}

fn machines_property(value: &FacetValue, span: Span) -> Result<FacetValue, EvalError> {
    if let Some(machine) = &value.machine {
        return Ok(FacetValue::scalar(machine.count.unwrap_or(1.0)));
    }
    if let Some(process) = &value.process {
        return match process.extent {
            None => Ok(FacetValue::scalar(1.0)),
            Some(Extent::Machines(count)) => Ok(FacetValue::scalar(count)),
            Some(Extent::MachineSeconds(_)) => Err(EvalError::new(
                span,
                "`.machines` is not defined for machine-seconds",
            )),
        };
    }
    Err(EvalError::new(
        span,
        "`.machines` needs a machine or process",
    ))
}

fn time_property(value: &FacetValue, span: Span) -> Result<FacetValue, EvalError> {
    let process = value
        .process
        .as_ref()
        .ok_or_else(|| EvalError::new(span, "`.time` needs a process"))?;
    let Some(Extent::MachineSeconds(seconds)) = process.extent else {
        return Err(EvalError::new(span, "`.time` needs machine-seconds"));
    };
    Ok(FacetValue::item(Monomial::new(
        seconds,
        [(BaseUnit::Second, 1)],
    )))
}

fn crafts_property(value: &FacetValue, span: Span) -> Result<FacetValue, EvalError> {
    let process = value
        .process
        .as_ref()
        .ok_or_else(|| EvalError::new(span, "`.crafts` needs a process"))?;
    let amount = process.net.get(&BaseUnit::Craft).copied().unwrap_or(0.0);
    let units = match process.extent {
        Some(Extent::MachineSeconds(_)) => vec![(BaseUnit::Craft, 1)],
        _ => vec![(BaseUnit::Craft, 1), (BaseUnit::Second, -1)],
    };
    Ok(FacetValue::item(Monomial::new(amount, units)))
}

fn machine_scalar(
    value: &FacetValue,
    property: Property,
    span: Span,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let machine = value
        .machine
        .as_ref()
        .ok_or_else(|| EvalError::new(span, "property needs a machine"))?;
    let data = registry
        .entry_by_index(machine.machine)
        .machine()
        .ok_or_else(|| EvalError::new(span, "not a machine"))?;
    let modifiers =
        facet_machine::modifiers(registry, machine.machine, &machine.config, None, settings);
    Ok(FacetValue::scalar(match property {
        Property::Speed => modifiers.speed,
        Property::Productivity | Property::Prod => modifiers.productivity,
        Property::Pollution => modifiers.pollution_per_minute,
        Property::Slots => f64::from(data.module_slots),
        _ => unreachable!(),
    }))
}

fn unit_selector(
    value: FacetValue,
    selector: &Selector,
    span: Span,
    registry: &Registry,
) -> Result<FacetValue, EvalError> {
    let Selector::Token(id) = selector else {
        unreachable!()
    };
    let process = value
        .process
        .ok_or_else(|| EvalError::new(span, "unit selectors need a process"))?;
    let unit = if registry.entry_by_index(*id).fluid {
        BaseUnit::Fluid(*id)
    } else {
        BaseUnit::Item(*id)
    };
    Ok(FacetValue::item(Monomial::new(
        process.net.get(&unit).copied().unwrap_or(0.0),
        [(unit, 1)],
    )))
}

fn watts(value: f64) -> FacetValue {
    FacetValue::item(Monomial::new(
        value,
        [(BaseUnit::Joule, 1), (BaseUnit::Second, -1)],
    ))
}

fn joules(value: f64) -> FacetValue {
    FacetValue::item(Monomial::new(value, [(BaseUnit::Joule, 1)]))
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatementOutput {
    pub line: usize,
    pub source_hash: u64,
    pub span: Span,
    pub output: LineOutput,
    pub value: Option<FacetValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineOutput {
    Nothing,
    Value(FacetValue),
    Error(EvalError),
}

pub fn line_hash(line: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    line.hash(&mut hasher);
    hasher.finish()
}

/// Evaluates every line in order. Parse failures do not bind names; evaluation
/// failures bind a poisoned value that later references propagate.
pub fn evaluate(text: &str, settings: &Settings, registry: &Registry) -> Vec<StatementOutput> {
    let mut variables = Variables::new();
    let mut outputs = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let statement = match crate::facet_parser::parse_line(line, registry) {
            Ok(statement) => statement,
            Err(error) => {
                outputs.push(StatementOutput {
                    line: index,
                    source_hash: line_hash(line),
                    span: error.span,
                    output: LineOutput::Error(EvalError::new(error.span, error.message)),
                    value: None,
                });
                continue;
            }
        };
        let result = eval_statement(&statement, &mut variables, registry, settings);
        let output = match result {
            None => LineOutput::Nothing,
            Some(Ok(value)) => LineOutput::Value(value.clone()),
            Some(Err(error)) => LineOutput::Error(error),
        };
        let value = match &output {
            LineOutput::Value(value) => Some(value.clone()),
            _ => None,
        };
        outputs.push(StatementOutput {
            line: index,
            source_hash: line_hash(line),
            span: statement.span,
            output,
            value,
        });
    }
    outputs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facet_parser::parse_line;
    use hactorio_data::{Locale, postprocess};

    const DUMP: &str = r#"{
      "item": {
        "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
        "iron-gear-wheel": {"type": "item", "name": "iron-gear-wheel", "stack_size": 100},
        "transport-belt": {"type": "item", "name": "transport-belt", "stack_size": 100},
        "steel-chest": {"type": "item", "name": "steel-chest", "stack_size": 50},
        "assembling-machine-3": {"type": "item", "name": "assembling-machine-3", "stack_size": 50},
        "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}},
        "beacon": {"type": "item", "name": "beacon", "stack_size": 20},
        "radar": {"type": "item", "name": "radar", "stack_size": 50},
        "accumulator": {"type": "item", "name": "accumulator", "stack_size": 50}
      },
      "fluid": {},
      "module": {
        "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}
      },
      "recipe": {
        "iron-gear-wheel": {"type": "recipe", "name": "iron-gear-wheel", "categories": ["crafting"], "energy_required": 1.0, "results": [{"type": "item", "name": "iron-gear-wheel", "amount": 1}]}
      },
      "transport-belt": {
        "transport-belt": {"type": "transport-belt", "name": "transport-belt", "speed": 0.03125}
      },
      "container": {
        "steel-chest": {"type": "container", "name": "steel-chest", "inventory_size": 48}
      },
      "assembling-machine": {
        "assembling-machine-3": {"type": "assembling-machine", "name": "assembling-machine-3", "crafting_categories": ["crafting"], "crafting_speed": 1.0, "module_slots": 4, "allowed_effects": ["speed"], "energy_usage": "100kW"}
      },
      "beacon": {
        "beacon": {"type": "beacon", "name": "beacon", "module_slots": 2, "distribution_effectivity": 1.5, "profile": [1.0], "energy_usage": "480kW"}
      },
      "radar": {
        "radar": {"type": "radar", "name": "radar", "energy_usage": "300kW"}
      },
      "accumulator": {
        "accumulator": {"type": "accumulator", "name": "accumulator", "buffer_capacity": "5MJ"}
      }
    }"#;

    fn registry() -> Registry {
        postprocess(DUMP, &Locale::empty()).unwrap()
    }

    fn eval(line: &str) -> Result<FacetValue, EvalError> {
        let registry = registry();
        let statement = parse_line(line, &registry).unwrap();
        match statement.kind {
            StatementKind::Expression(expr) => {
                eval_expr(&expr, &Variables::new(), &registry, &Settings::default())
            }
            other => panic!("expected expression, got {other:?}"),
        }
    }

    #[test]
    fn evaluates_arithmetic() {
        assert_eq!(eval("2 + 3").unwrap().item.unwrap().coef(), 5.0);
        assert_eq!(
            eval("3 :iron-plate:/s * 8 s").unwrap().item.unwrap().coef(),
            24.0
        );
    }

    #[test]
    fn evaluates_rate_and_capacity_application() {
        let value = eval(":transport-belt: :iron-plate:").unwrap();
        assert_eq!(value.item.unwrap().coef(), 15.0);
        let value = eval(":steel-chest: :iron-plate:").unwrap();
        assert_eq!(value.item.unwrap().coef(), 4800.0);
    }

    #[test]
    fn evaluates_stack_size() {
        assert_eq!(
            eval(":stack-size: :iron-plate:")
                .unwrap()
                .item
                .unwrap()
                .coef(),
            100.0
        );
        let value = eval(":stack-size: :transport-belt:").unwrap();
        assert_eq!(value.item.unwrap().coef(), 100.0);
        assert_eq!(value.belt.unwrap().cap, Cap::Limit(4));
        let value = eval(":stack-size: :transport-belt: :iron-plate:").unwrap();
        assert_eq!(value.item.unwrap().coef(), 60.0);
    }

    #[test]
    fn evaluates_config_juxtaposition() {
        let value = eval(":beacon: 2 :speed-module-3:").unwrap();
        assert!(value.item.is_some());
        assert_eq!(value.config.unwrap().beacons[0].modules[0].count, 2);

        let value = eval(":speed-module-3: 2 :speed-module-3:").unwrap();
        assert!(value.item.is_none());
        assert_eq!(value.config.unwrap().modules[0].count, 3);

        assert!(eval(":speed-module-3: :beacon:").is_err());
    }

    #[test]
    fn evaluates_with_making_and_properties() {
        let value = eval(
            "(2 :assembling-machine-3: with 2 :speed-module-3: making :iron-gear-wheel:).machines",
        )
        .unwrap();
        assert_eq!(value.item.unwrap().coef(), 2.0);

        let value = eval(":assembling-machine-3: with 2 :speed-module-3: making :iron-gear-wheel:")
            .unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.extent, None);
        assert_eq!(process.net[&BaseUnit::Craft], 2.0);
    }

    #[test]
    fn evaluates_in_and_process_properties() {
        let value = eval(":iron-gear-wheel: in :assembling-machine-3:").unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.extent, Some(Extent::MachineSeconds(1.0)));

        let value = eval(":iron-gear-wheel:/s in :assembling-machine-3:").unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.extent, Some(Extent::Machines(1.0)));

        let value = eval("1 craft in (:assembling-machine-3: making :iron-gear-wheel:)").unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.extent, Some(Extent::MachineSeconds(1.0)));

        assert!(eval(":iron-gear-wheel: in (1 :assembling-machine-3:)").is_err());
    }

    #[test]
    fn evaluates_properties() {
        assert_eq!(
            eval("(:transport-belt:).capacity")
                .unwrap()
                .capacity
                .unwrap(),
            CapacityFacet::Items(BTreeMap::from([(Cap::Limit(1), 8.0)]))
        );
        assert_eq!(
            eval("(:assembling-machine-3:).slots")
                .unwrap()
                .item
                .unwrap()
                .coef(),
            4.0
        );
        assert_eq!(
            eval("(2 :assembling-machine-3:).machines")
                .unwrap()
                .item
                .unwrap()
                .coef(),
            2.0
        );
        assert_eq!(
            eval("(:steel-chest:).capacity").unwrap().capacity.unwrap(),
            CapacityFacet::Items(BTreeMap::from([(Cap::Full, 48.0)]))
        );
    }

    #[test]
    fn config_division_drops_config() {
        let value = eval(":speed-module-3: / 2").unwrap();
        assert_eq!(value.item.unwrap().coef(), 0.5);
        assert!(value.config.is_none());
        assert!(eval("(:speed-module-3: + :beacon:) / 2").is_err());
    }
}
