//! Evaluation of the facet syntax tree.

use std::collections::{BTreeMap, HashMap};

use hactorio_data::{EntryId, Registry};
use thiserror::Error;

use crate::facet_machine;
use crate::facet_parser::{
    AssignmentTarget, DefinitionOp, Expr, ExprKind, Property, Selector, Signal, Statement,
    StatementKind,
};
use crate::facet_token::token_value;
use crate::facets::{
    Cap, CapacityFacet, ExtentKind, FacetError, FacetValue, Monomial, ProcessFacet, RateFacet,
};
use crate::settings::Settings;
use crate::syntax::{BinaryOp, Span};
use crate::value::{BaseUnit, is_nonzero, is_zero};

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
        ExprKind::Variable(name) => variable_value(variables, name, span),
        ExprKind::Neg(inner) => {
            let value = eval_expr(inner, variables, registry, settings)?;
            map_unary(value.neg(), span, &value)
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let lhs = eval_expr(lhs, variables, registry, settings)?;
            let rhs = eval_expr(rhs, variables, registry, settings)?;
            apply_binary(*op, &lhs, &rhs, span)
        }
        ExprKind::Juxtapose { lhs, rhs } => {
            let lhs = eval_expr(lhs, variables, registry, settings)?;
            let rhs = eval_expr(rhs, variables, registry, settings)?;
            let stack = item_stack_size(&rhs, registry);
            let value = lhs.juxtapose(&rhs, stack).map_err(|error| {
                EvalError::new(span, explain_failure("juxtaposition", &lhs, &rhs, error))
            })?;
            if let Some(config) = &value.config {
                facet_machine::validate_beacon_groups(registry, config)
                    .map_err(|error| EvalError::new(span, error))?;
            }
            Ok(value)
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
        ExprKind::StackedBelt(inner) => {
            let value = eval_expr(inner, variables, registry, settings)?;
            stacked_belt(value, span, settings)
        }
        ExprKind::Property { value, selector } => {
            let value = eval_expr(value, variables, registry, settings)?;
            read_property(value, selector, span, registry, settings)
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
        StatementKind::Definition { target, op, value } => {
            let (name, result) = match target {
                AssignmentTarget::Variable(name) => {
                    let result = match op {
                        DefinitionOp::Assign => eval_expr(value, variables, registry, settings),
                        DefinitionOp::Compound(binary) => {
                            variable_value(variables, name, statement.span).and_then(|current| {
                                let value = eval_expr(value, variables, registry, settings)?;
                                apply_binary(*binary, &current, &value, statement.span)
                            })
                        }
                    };
                    (name, result)
                }
                AssignmentTarget::Property { variable, selector } => {
                    let result =
                        variable_value(variables, variable, statement.span).and_then(|current| {
                            let rhs = eval_expr(value, variables, registry, settings)?;
                            assign_property(current, selector, *op, &rhs, statement.span, registry)
                        });
                    (variable, result)
                }
            };
            variables.insert(name.clone(), result.clone());
            Some(result)
        }
    }
}

fn variable_value(variables: &Variables, name: &str, span: Span) -> Result<FacetValue, EvalError> {
    match variables.get(name) {
        Some(Ok(value)) => Ok(value.clone()),
        Some(Err(error)) => Err(error.clone()),
        None => Err(EvalError::new(span, format!("unknown variable `{name}`"))),
    }
}

/// The coefficient of a value that is exactly the monomial `units`.
///
/// Assignment targets take a bare amount, so any other facet means the value is
/// not an amount at all.
fn monomial_amount(value: &FacetValue, units: &[(BaseUnit, i64)]) -> Option<f64> {
    if value.recipe.is_some()
        || value.machine.is_some()
        || value.process.is_some()
        || value.config.is_some()
        || value.belt.is_some()
        || value.rate.is_some()
        || value.capacity.is_some()
    {
        return None;
    }
    let item = value.item.as_ref()?;
    if item.units().len() != units.len() {
        return None;
    }
    units
        .iter()
        .all(|(unit, exponent)| item.units().get(unit) == Some(exponent))
        .then(|| item.coef())
}

/// The scalar of a value usable as the factor of `*=` or `/=`.
fn dimensionless_amount(value: &FacetValue) -> Option<f64> {
    monomial_amount(value, &[])
}

/// How a unit is spelled in an assignment error message.
fn unit_label(unit: BaseUnit, registry: &Registry) -> String {
    match unit {
        BaseUnit::Item(id) | BaseUnit::Fluid(id) => {
            format!(":{}:", registry.entry_by_index(id).id)
        }
        BaseUnit::Craft => "craft".to_string(),
        BaseUnit::Second => "s".to_string(),
        BaseUnit::Joule => "J".to_string(),
    }
}

/// The coefficient of a process net amount: per second, or per machine-second.
fn net_amount(
    value: &FacetValue,
    unit: BaseUnit,
    rate: bool,
    span: Span,
    registry: &Registry,
) -> Result<f64, EvalError> {
    let found = if rate {
        monomial_amount(value, &[(BaseUnit::Second, -1)])
            .or_else(|| monomial_amount(value, &[(unit, 1), (BaseUnit::Second, -1)]))
    } else {
        monomial_amount(value, &[]).or_else(|| monomial_amount(value, &[(unit, 1)]))
    };
    let label = unit_label(unit, registry);
    found.ok_or_else(|| {
        EvalError::new(
            span,
            if rate {
                format!("expected `{label}/s` or `/s`")
            } else {
                format!("expected `{label}`")
            },
        )
    })
}

/// The watts a value holds, if it is exactly a power.
fn watt_amount(value: &FacetValue) -> Option<f64> {
    monomial_amount(value, &[(BaseUnit::Joule, 1), (BaseUnit::Second, -1)])
}

/// The joules a value holds, if it is exactly an energy.
fn joule_amount(value: &FacetValue) -> Option<f64> {
    monomial_amount(value, &[(BaseUnit::Joule, 1)])
}

/// The right-hand side of `op`: the scalar for `*=` and `/=`, else `amount`.
fn rhs_amount(
    op: DefinitionOp,
    scalar: Option<f64>,
    amount: impl FnOnce() -> Result<f64, EvalError>,
) -> Result<f64, EvalError> {
    match (op, scalar) {
        (DefinitionOp::Compound(BinaryOp::Mul | BinaryOp::Div), Some(scalar)) => Ok(scalar),
        _ => amount(),
    }
}

/// Combines a stored amount with the right-hand side, per operator.
fn combine(op: DefinitionOp, old: f64, amount: f64) -> f64 {
    match op {
        DefinitionOp::Assign => amount,
        DefinitionOp::Compound(BinaryOp::Add) => old + amount,
        DefinitionOp::Compound(BinaryOp::Sub) => old - amount,
        DefinitionOp::Compound(BinaryOp::Mul) => old * amount,
        DefinitionOp::Compound(BinaryOp::Div) => old / amount,
        DefinitionOp::Compound(BinaryOp::Pow) => unreachable!("`^=` is not a definition operator"),
    }
}

/// Writes `rhs` into the process facet `selector` names, per `op`.
///
/// The write mirrors the read rules: two machine-seconds fields and the net
/// coefficient keep their units, and zero keeps the field present.
fn assign_property(
    mut value: FacetValue,
    selector: &Selector,
    op: DefinitionOp,
    rhs: &FacetValue,
    span: Span,
    registry: &Registry,
) -> Result<FacetValue, EvalError> {
    let process = value
        .process
        .as_mut()
        .ok_or_else(|| EvalError::new(span, "process net assignment needs a process"))?;
    let scalar = match op {
        DefinitionOp::Compound(BinaryOp::Mul | BinaryOp::Div) => Some(
            dimensionless_amount(rhs)
                .ok_or_else(|| EvalError::new(span, "`*=` and `/=` need a dimensionless number"))?,
        ),
        _ => None,
    };
    match selector {
        Selector::Property(Property::Power) => {
            if process.is_machine_seconds() {
                return Err(EvalError::new(
                    span,
                    "`.power` is not defined for machine-seconds",
                ));
            }
            let amount = rhs_amount(op, scalar, || {
                watt_amount(rhs)
                    .ok_or_else(|| EvalError::new(span, "expected a power value such as `100 kW`"))
            })?;
            process.electric = combine(op, process.electric, amount);
        }
        Selector::Signal(Signal::Electric) => {
            let amount = if process.is_machine_seconds() {
                rhs_amount(op, scalar, || {
                    joule_amount(rhs).ok_or_else(|| {
                        EvalError::new(span, "expected an energy value such as `100 kJ`")
                    })
                })?
            } else {
                rhs_amount(op, scalar, || {
                    watt_amount(rhs).ok_or_else(|| {
                        EvalError::new(span, "expected a power value such as `100 kW`")
                    })
                })?
            };
            process.electric = combine(op, process.electric, amount);
        }
        Selector::Property(Property::Crafts) => {
            if process.machines.len() > 1 {
                return Err(EvalError::new(
                    span,
                    "`.crafts` cannot be assigned to a process with multiple machine configurations",
                ));
            }
            let rate = !process.is_machine_seconds();
            let amount = rhs_amount(op, scalar, || {
                net_amount(rhs, BaseUnit::Craft, rate, span, registry)
            })?;
            write_net(process, BaseUnit::Craft, op, amount);
        }
        Selector::Token(id) => {
            let unit = if registry.entry_by_index(*id).fluid {
                BaseUnit::Fluid(*id)
            } else {
                BaseUnit::Item(*id)
            };
            let rate = !process.is_machine_seconds();
            let amount = rhs_amount(op, scalar, || net_amount(rhs, unit, rate, span, registry))?;
            write_net(process, unit, op, amount);
        }
        Selector::Property(_) | Selector::Signal(_) => {
            unreachable!("parser rejects non-assignable selectors")
        }
    }
    Ok(value)
}

/// Writes `amount` into the process net row `unit`, per `op`.
///
/// `*=` and `/=` leave an absent row absent, so an empty net never gains a row
/// from a factor alone.
fn write_net(process: &mut ProcessFacet, unit: BaseUnit, op: DefinitionOp, amount: f64) {
    let old = process.net.get(&unit).copied().unwrap_or(0.0);
    if matches!(op, DefinitionOp::Compound(BinaryOp::Mul | BinaryOp::Div)) && is_zero(old) {
        return;
    }
    process.net.insert(unit, combine(op, old, amount));
    process.canonicalize();
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

const FACETS: [&str; 8] = [
    "item", "rate", "capacity", "machine", "process", "config", "belt", "recipe",
];

fn facet_present(value: &FacetValue, facet: &str) -> bool {
    match facet {
        "item" => value.item.is_some(),
        "rate" => value.rate.is_some(),
        "capacity" => value.capacity.is_some(),
        "machine" => value.machine.is_some(),
        "process" => value.process.is_some(),
        "config" => value.config.is_some(),
        "belt" => value.belt.is_some(),
        "recipe" => value.recipe.is_some(),
        _ => false,
    }
}

fn facet_names(value: &FacetValue) -> String {
    FACETS
        .iter()
        .filter(|facet| facet_present(value, facet))
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

fn drop_reason(op: &str, facet: &str, lhs: &FacetValue, rhs: &FacetValue) -> &'static str {
    let left = facet_present(lhs, facet);
    let right = facet_present(rhs, facet);
    let missing_counterpart = if left {
        "missing counterpart on right"
    } else {
        "missing counterpart on left"
    };
    match facet {
        "recipe" => "unsupported operation",
        "item" => {
            if left && right {
                if matches!(op, "+" | "-")
                    && lhs.item.as_ref().map(|m| m.units()) != rhs.item.as_ref().map(|m| m.units())
                {
                    "incompatible values"
                } else {
                    "unsupported operation"
                }
            } else {
                missing_counterpart
            }
        }
        "machine" => {
            if (left && right) || matches!(op, "*" | "/") {
                "incompatible values"
            } else {
                missing_counterpart
            }
        }
        "config" => {
            if (left && right) || op == "/" {
                "unsupported operation"
            } else if matches!(op, "*" | "juxtaposition") {
                "incompatible values"
            } else {
                missing_counterpart
            }
        }
        "belt" | "rate" | "capacity" | "process" => {
            if left && right {
                "incompatible values"
            } else {
                missing_counterpart
            }
        }
        _ => "unsupported operation",
    }
}

fn explain_failure(op: &str, lhs: &FacetValue, rhs: &FacetValue, error: FacetError) -> String {
    if !matches!(
        error,
        FacetError::NoFacetSurvives | FacetError::UnsupportedOperation
    ) {
        return error.to_string();
    }
    if op == "juxtaposition" && error == FacetError::UnsupportedOperation {
        return format!(
            "juxtaposition matched no rule (left: [{}], right: [{}]); use explicit `*` between unit-bearing values",
            facet_names(lhs),
            facet_names(rhs)
        );
    }
    let reasons: Vec<String> = FACETS
        .iter()
        .filter(|facet| facet_present(lhs, facet) || facet_present(rhs, facet))
        .map(|facet| format!("{facet}: {}", drop_reason(op, facet, lhs, rhs)))
        .collect();
    format!(
        "`{op}` dropped every facet (left: [{}], right: [{}])\n{}",
        facet_names(lhs),
        facet_names(rhs),
        reasons.join("\n")
    )
}

fn explain_unary(value: &FacetValue, error: FacetError) -> String {
    if !matches!(
        error,
        FacetError::NoFacetSurvives | FacetError::UnsupportedOperation
    ) {
        return error.to_string();
    }
    let reasons: Vec<String> = FACETS
        .iter()
        .filter(|facet| facet_present(value, facet))
        .map(|facet| {
            let reason = if matches!(*facet, "config" | "recipe") {
                "unsupported operation"
            } else {
                "missing counterpart"
            };
            format!("{facet}: {reason}")
        })
        .collect();
    format!(
        "unary `-` dropped every facet (operand: [{}]); {}",
        facet_names(value),
        reasons.join("; ")
    )
}

fn apply_binary(
    op: BinaryOp,
    lhs: &FacetValue,
    rhs: &FacetValue,
    span: Span,
) -> Result<FacetValue, EvalError> {
    let (result, name) = match op {
        BinaryOp::Add => (lhs.add(rhs), "+"),
        BinaryOp::Sub => (lhs.sub(rhs), "-"),
        BinaryOp::Mul => (lhs.mul(rhs), "*"),
        BinaryOp::Div => (lhs.div(rhs), "/"),
        BinaryOp::Pow => (lhs.pow(rhs), "^"),
    };
    map_binary(result, span, name, lhs, rhs)
}

fn map_binary(
    result: Result<FacetValue, FacetError>,
    span: Span,
    op: &str,
    lhs: &FacetValue,
    rhs: &FacetValue,
) -> Result<FacetValue, EvalError> {
    result.map_err(|error| EvalError::new(span, explain_failure(op, lhs, rhs, error)))
}

fn map_unary(
    result: Result<FacetValue, FacetError>,
    span: Span,
    value: &FacetValue,
) -> Result<FacetValue, EvalError> {
    result.map_err(|error| EvalError::new(span, explain_unary(value, error)))
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
    )
    .map_err(|error| EvalError::new(span, error))?;
    let spec = machine.spec();
    let (kind, machines, net, electric) = match machine.count {
        None => (
            ExtentKind::Unsolved,
            BTreeMap::from([(spec, 1.0)]),
            net,
            modifiers.power_w,
        ),
        Some(count) => (
            ExtentKind::Machines,
            BTreeMap::from([(spec, count)]),
            net.into_iter()
                .map(|(unit, amount)| (unit, amount * count))
                .collect(),
            modifiers.power_w * count,
        ),
    };
    let mut process = ProcessFacet {
        kind,
        machines,
        net,
        electric,
    };
    process.canonicalize();
    Ok(FacetValue {
        process: Some(process),
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
    let mut unit = None;
    let mut second = false;
    for (base, exponent) in item.units() {
        match (base, *exponent) {
            (BaseUnit::Second, -1) if !second => second = true,
            (BaseUnit::Item(_) | BaseUnit::Fluid(_) | BaseUnit::Craft, 1) if unit.is_none() => {
                unit = Some(*base);
            }
            _ => return None,
        }
    }
    let unit = unit?;
    Some(InQuantity {
        unit,
        coefficient: item.coef(),
        rate: second,
        craft: unit == BaseUnit::Craft,
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
    let (net, power, spec) = if let Some(process) = &machine.process {
        if !process.is_unsolved() {
            return Err(EvalError::new(span, "`in` cannot use a solved process"));
        }
        let net = process.net.clone();
        let power = process.electric;
        let spec =
            process.machines.keys().next().cloned().ok_or_else(|| {
                EvalError::new(span, "`in` needs an unsolved process with a machine")
            })?;
        (net, power, spec)
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
        )
        .map_err(|error| EvalError::new(span, error))?;
        (net, modifiers.power_w, machine_facet.spec())
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
    if is_zero(denominator) {
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
    let kind = if quantity.rate {
        ExtentKind::Machines
    } else {
        ExtentKind::MachineSeconds
    };
    let mut process = ProcessFacet {
        kind,
        machines: BTreeMap::from([(spec, magnitude)]),
        net: net
            .into_iter()
            .map(|(unit, amount)| (unit, amount * magnitude))
            .collect(),
        electric: power * magnitude,
    };
    process.canonicalize();
    Ok(FacetValue {
        process: Some(process),
        ..FacetValue::empty()
    })
}

fn stack_size(
    value: FacetValue,
    span: Span,
    registry: &Registry,
    _settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let item = value
        .item
        .ok_or_else(|| EvalError::new(span, "`:stack-size:` needs an item"))?;
    let (unit, exponent) = item
        .units()
        .iter()
        .next()
        .ok_or_else(|| EvalError::new(span, "`:stack-size:` needs an item"))?;
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

fn stacked_belt(
    value: FacetValue,
    span: Span,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    if value.belt.is_none() {
        return Err(EvalError::new(span, "`:stacked-belt:` needs a belt"));
    }
    let rate = value
        .rate
        .ok_or_else(|| EvalError::new(span, "`:stacked-belt:` needs a belt rate"))?;
    let level = settings.belt_stacking.clamp(1, 4);
    let cap = Cap::Limit(u32::from(level));
    Ok(FacetValue {
        rate: Some(cap_rate(rate, cap)),
        ..FacetValue::empty()
    })
}

fn cap_rate(rate: RateFacet, cap: Cap) -> RateFacet {
    match rate {
        RateFacet::Items(items) => RateFacet::Items(
            [(cap, items.values().copied().sum::<f64>())]
                .into_iter()
                .filter(|(_, amount)| is_nonzero(*amount))
                .collect(),
        ),
        RateFacet::Fluid(value) => RateFacet::Fluid(value),
    }
}

/// Whether `property` can be read from `value`, by the same rules `eval` uses.
pub fn property_is_available(
    value: &FacetValue,
    property: Property,
    registry: &Registry,
    settings: &Settings,
) -> bool {
    read_property(
        value.clone(),
        &Selector::Property(property),
        Span::new(0, 0),
        registry,
        settings,
    )
    .is_ok()
}

fn read_property(
    value: FacetValue,
    selector: &Selector,
    span: Span,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, EvalError> {
    let property = match selector {
        Selector::Property(property) => *property,
        Selector::Signal(Signal::Time) => Property::Time,
        Selector::Signal(Signal::Speed) => Property::Speed,
        Selector::Signal(Signal::Electric) => Property::Electric,
        Selector::Signal(Signal::ModuleSlots) => Property::Slots,
        Selector::Signal(Signal::StackSize) => Property::StackSize,
        Selector::Signal(Signal::StackedBelt) => {
            return Err(EvalError::new(
                span,
                "`:stacked-belt:` is a prefix, not a property",
            ));
        }
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
                .ok_or_else(|| EvalError::new(span, "`.:stack-size:` needs an item"))?;
            let (unit, exponent) = item
                .units()
                .iter()
                .next()
                .ok_or_else(|| EvalError::new(span, "`.:stack-size:` needs an item"))?;
            if *exponent != 1 || item.units().len() != 1 {
                return Err(EvalError::new(span, "`.:stack-size:` needs a pure item"));
            }
            let BaseUnit::Item(id) = unit else {
                return Err(EvalError::new(span, "`.:stack-size:` needs an item"));
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
            facet_machine::modifiers(registry, machine.machine, &machine.config, None, settings)
                .map_err(|error| EvalError::new(span, error))?;
        let count = machine.count.unwrap_or(1.0);
        return Ok(watts(modifiers.power_w * count));
    }
    if let Some(process) = &value.process {
        if process.is_machine_seconds() {
            if electric {
                return Ok(joules(process.electric));
            }
            return Err(EvalError::new(
                span,
                "`.power` is not defined for machine-seconds",
            ));
        }
        return Ok(watts(process.electric));
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
                "`.:electric:` is ambiguous: the item has both power and stored energy",
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
        return Err(EvalError::new(span, "no power for this value"));
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
    Err(EvalError::new(span, "no power for this value"))
}

fn energy_property(
    value: &FacetValue,
    span: Span,
    registry: &Registry,
) -> Result<FacetValue, EvalError> {
    if let Some(process) = &value.process {
        if process.is_machine_seconds() {
            return Ok(joules(process.electric));
        }
        return Err(EvalError::new(span, "process has no energy"));
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
        if process.is_machine_seconds() {
            return Err(EvalError::new(
                span,
                "`.machines` is not defined for machine-seconds",
            ));
        }
        return Ok(FacetValue::scalar(process.magnitude()));
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
        .ok_or_else(|| EvalError::new(span, "`.:time:` needs a process"))?;
    if !process.is_machine_seconds() {
        return Err(EvalError::new(span, "`.:time:` needs machine-seconds"));
    }
    Ok(FacetValue::item(Monomial::new(
        process.magnitude(),
        [(BaseUnit::Second, 1)],
    )))
}

fn crafts_property(value: &FacetValue, span: Span) -> Result<FacetValue, EvalError> {
    let process = value
        .process
        .as_ref()
        .ok_or_else(|| EvalError::new(span, "`.crafts` needs a process"))?;
    let amount = process.net.get(&BaseUnit::Craft).copied().unwrap_or(0.0);
    let units = if process.is_machine_seconds() {
        vec![(BaseUnit::Craft, 1)]
    } else {
        vec![(BaseUnit::Craft, 1), (BaseUnit::Second, -1)]
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
        facet_machine::modifiers(registry, machine.machine, &machine.config, None, settings)
            .map_err(|error| EvalError::new(span, error))?;
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
    let rate = !process.is_machine_seconds();
    let units = if rate {
        vec![(unit, 1), (BaseUnit::Second, -1)]
    } else {
        vec![(unit, 1)]
    };
    Ok(FacetValue::item(Monomial::new(
        process.net.get(&unit).copied().unwrap_or(0.0),
        units,
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

/// Evaluates every line in order. Parse failures do not bind names. A definition
/// replaces any earlier binding of the same name. Evaluation failures bind a
/// poisoned value that later references propagate until the name is defined again.
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
    use crate::facets::ExtentKind;
    use hactorio_data::{Locale, postprocess};

    const DUMP: &str = r#"{
      "item": {
        "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
        "copper-plate": {"type": "item", "name": "copper-plate", "stack_size": 100},
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
        "beacon": {"type": "beacon", "name": "beacon", "module_slots": 2, "allowed_effects": ["speed"], "distribution_effectivity": 1.5, "profile": [1.0], "energy_usage": "480kW"}
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
        let item = value.item.unwrap();
        assert_eq!(item.coef(), 15.0);
        assert_eq!(item.exponent(BaseUnit::Second), -1);
        let value = eval(":steel-chest: :iron-plate:").unwrap();
        let item = value.item.unwrap();
        assert_eq!(item.coef(), 4800.0);
        assert_eq!(item.exponent(BaseUnit::Second), 0);
    }

    #[test]
    fn applies_rates_as_rates() {
        let value = eval(":transport-belt: :iron-plate:").unwrap();
        let item = value.item.unwrap();
        assert_eq!(item.units().len(), 2);
        assert_eq!(item.exponent(BaseUnit::Second), -1);
        // A rate has no item mode for the stack-size prefix.
        assert!(eval(":stack-size: (:transport-belt: :iron-plate:)").is_err());
    }

    #[test]
    fn accepts_craft_rates_in() {
        let value = eval("1 craft/s in (:assembling-machine-3: making :iron-gear-wheel:)").unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.kind, ExtentKind::Machines);
        assert_eq!(process.magnitude(), 1.0);
    }

    #[test]
    fn validates_beacon_groups_at_juxtaposition() {
        assert!(eval(":beacon: 2 :speed-module-3:").is_ok());
        assert!(eval(":beacon: 3 :speed-module-3:").is_err());
    }

    #[test]
    fn evaluates_stored_energy_in_joules() {
        let value = eval(":accumulator:.energy").unwrap();
        let item = value.item.unwrap();
        assert_eq!(item.exponent(BaseUnit::Joule), 1);
        assert_eq!(item.coef(), 5_000_000.0);
        let value = eval(":accumulator:.:electric:").unwrap();
        assert_eq!(value.item.unwrap().coef(), 5_000_000.0);
        assert!(eval(":accumulator:.power").is_err());
    }

    #[test]
    fn explains_dropped_facets() {
        let error = eval("2 :iron-plate: + 3 :copper-plate:").unwrap_err();
        assert!(error.message.contains("`+`"), "{}", error.message);
        assert!(error.message.contains("item"), "{}", error.message);
        assert!(error.message.contains("incompatible"), "{}", error.message);

        let error = eval("2 :iron-plate: 3 :copper-plate:").unwrap_err();
        assert!(error.message.contains("juxtaposition"), "{}", error.message);
        assert!(error.message.contains("explicit `*`"), "{}", error.message);

        let error = eval("(:assembling-machine-3: with 2 :speed-module-3:) * 2 s").unwrap_err();
        assert!(error.message.contains("`*`"), "{}", error.message);
        assert!(error.message.contains("machine"), "{}", error.message);
        assert!(error.message.contains("item"), "{}", error.message);
    }

    #[test]
    fn evaluates_stack_size_and_stacked_belt() {
        assert_eq!(
            eval(":stack-size: :iron-plate:")
                .unwrap()
                .item
                .unwrap()
                .coef(),
            100.0
        );
        let value = eval(":stack-size: :transport-belt:").unwrap();
        let item = value.item.unwrap();
        assert_eq!(item.coef(), 100.0);
        assert!(value.belt.is_none());
        assert!(value.rate.is_none());
        assert!(eval(":stack-size: :transport-belt: :iron-plate:").is_err());

        let value = eval(":stacked-belt: :transport-belt:").unwrap();
        assert!(value.item.is_none());
        assert!(value.belt.is_none());
        assert_eq!(
            value.rate,
            Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(4), 15.0)])))
        );

        let value = eval(":stacked-belt: :transport-belt: :iron-plate:").unwrap();
        let item = value.item.unwrap();
        assert_eq!(item.coef(), 60.0);
        assert_eq!(item.exponent(BaseUnit::Second), -1);

        let value = eval("2 :stacked-belt: :transport-belt:").unwrap();
        assert_eq!(
            value.rate,
            Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(4), 30.0)])))
        );

        let value = eval(":stacked-belt: (:transport-belt: + :transport-belt:)").unwrap();
        assert_eq!(
            value.rate,
            Some(RateFacet::Items(BTreeMap::from([(Cap::Limit(4), 30.0)])))
        );
        assert!(eval(":stacked-belt: :iron-plate:").is_err());
        assert!(eval(":stacked-belt: :transport-belt:.rate").is_err());
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
        assert!(process.is_unsolved());
        assert_eq!(process.net[&BaseUnit::Craft], 2.0);
    }

    #[test]
    fn abs_is_not_defined_for_processes() {
        assert!(eval("(:iron-plate: in :assembling-machine-3:).abs").is_err());
        assert!(eval("2 :iron-plate:.abs").is_ok());
    }

    #[test]
    fn rejects_beacon_count_beyond_the_distribution_profile() {
        assert!(eval(":assembling-machine-3: with :beacon: making :iron-gear-wheel:").is_ok());
        let error =
            eval(":assembling-machine-3: with 2 :beacon: making :iron-gear-wheel:").unwrap_err();
        assert_eq!(
            error.to_string(),
            "2 beacons exceed the `beacon` distribution profile length (1)"
        );
    }

    #[test]
    fn evaluates_in_and_process_properties() {
        let value = eval(":iron-gear-wheel: in :assembling-machine-3:").unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.kind, ExtentKind::MachineSeconds);
        assert_eq!(process.magnitude(), 1.0);

        let value = eval(":iron-gear-wheel:/s in :assembling-machine-3:").unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.kind, ExtentKind::Machines);
        assert_eq!(process.magnitude(), 1.0);

        let value = eval("1 craft in (:assembling-machine-3: making :iron-gear-wheel:)").unwrap();
        let process = value.process.unwrap();
        assert_eq!(process.kind, ExtentKind::MachineSeconds);
        assert_eq!(process.magnitude(), 1.0);

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
            eval("(:assembling-machine-3:).:module-slots:")
                .unwrap()
                .item
                .unwrap()
                .coef(),
            4.0
        );
        assert!(parse_line("(:assembling-machine-3:).slots", &registry()).is_err());
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
