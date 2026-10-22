//! Evaluation of parsed statements.

use std::collections::{BTreeMap, HashMap};

use hactorio_data::{EntryId, RecipeId, Registry};
use thiserror::Error;

use crate::ast::{BinaryOp, Expr, MachineSpec, ModuleUse, Span, StatementKind};
use crate::belt::belt_value;
use crate::config;
use crate::line_cache::LineCache;
use crate::settings::Settings;
use crate::value::{BaseUnit, Value, ValueError};

/// A semantic failure, reported on the statement that caused it.
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

/// The value of one expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Eval {
    Scalar(Value),
    Machine(MachineValue),
    Recipe(RecipeValue),
}

/// Machines with modules, before a recipe is attached.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineValue {
    pub machine: EntryId,
    /// Modules merged by id.
    pub modules: Vec<ModuleCount>,
    pub count: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCount {
    pub module: EntryId,
    pub count: f64,
}

/// Machines with modules and one recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeValue {
    pub machine: EntryId,
    /// Modules merged by id.
    pub modules: Vec<ModuleCount>,
    pub recipe: RecipeId,
    pub quantity: RecipeQuantity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecipeQuantity {
    /// A machine count.
    Machines(f64),
    /// Machine time, `machine * second`.
    MachineSeconds(f64),
}

impl RecipeQuantity {
    pub fn magnitude(self) -> f64 {
        match self {
            RecipeQuantity::Machines(value) | RecipeQuantity::MachineSeconds(value) => value,
        }
    }
}

/// The output of one line.
#[derive(Debug, Clone, PartialEq)]
pub struct StatementOutput {
    pub line: usize,
    /// Hash of the source text this output was computed from.
    pub source_hash: u64,
    pub span: Span,
    pub output: LineOutput,
    /// The evaluated value, kept so the focused line can show a breakdown.
    pub eval: Option<Eval>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineOutput {
    Nothing,
    Value(Output),
    Error(EvalError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Scalar(Value),
    Machine(MachineOutput),
    Recipe(RecipeOutput),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MachineOutput {
    pub machine: EntryId,
    pub count: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecipeOutput {
    pub machine: EntryId,
    pub quantity: RecipeQuantity,
    /// Net amounts per base unit: rates for `Machines`, totals for
    /// `MachineSeconds`. Rows are signed and never zero.
    pub net: BTreeMap<BaseUnit, f64>,
    /// Active power of one machine, in watts.
    pub per_machine_power_w: f64,
    /// Total energy in joules; present for `MachineSeconds` quantities.
    pub total_energy_j: Option<f64>,
}

/// Evaluates one tab.
///
/// Variables are local to the tab. Blank and comment lines produce no output.
/// A definition that fails binds a poisoned value, and later references report
/// the original message.
pub fn evaluate(
    tab: &str,
    settings: &Settings,
    registry: &Registry,
    cache: &mut LineCache,
) -> Vec<StatementOutput> {
    let lines: Vec<&str> = tab.split('\n').collect();
    let mut variables: HashMap<String, Result<Eval, EvalError>> = HashMap::new();
    let mut outputs = Vec::with_capacity(lines.len());
    let mut seen = std::collections::HashSet::with_capacity(lines.len());

    for (index, line) in lines.iter().enumerate() {
        seen.insert(*line);
        let (span, output, eval) = match cache.statement(line, registry) {
            Err(error) => (
                error.span,
                LineOutput::Error(EvalError {
                    span: error.span,
                    message: error.message,
                }),
                None,
            ),
            Ok(statement) => {
                let span = statement.span;
                let (output, eval) =
                    evaluate_statement(&statement.kind, &mut variables, settings, registry, span);
                (span, output, eval)
            }
        };
        outputs.push(StatementOutput {
            line: index,
            source_hash: line_hash(line),
            span,
            output,
            eval,
        });
    }

    cache.retain(&seen);
    outputs
}

fn evaluate_statement(
    kind: &StatementKind,
    variables: &mut HashMap<String, Result<Eval, EvalError>>,
    settings: &Settings,
    registry: &Registry,
    span: Span,
) -> (LineOutput, Option<Eval>) {
    match kind {
        StatementKind::Blank | StatementKind::Comment => (LineOutput::Nothing, None),
        StatementKind::Expression(expr) => {
            match eval_expr(expr, variables, settings, registry, span) {
                Ok(value) => {
                    let output = match output_of(&value, settings, registry, span) {
                        Ok(output) => LineOutput::Value(output),
                        Err(error) => LineOutput::Error(error),
                    };
                    (output, Some(value))
                }
                Err(error) => (LineOutput::Error(error), None),
            }
        }
        StatementKind::Definition { name, value } => {
            if variables.contains_key(name) {
                return (
                    LineOutput::Error(EvalError::new(span, format!("`{name}` is already defined"))),
                    None,
                );
            }
            let result = eval_expr(value, variables, settings, registry, span);
            let output = match &result {
                Ok(value) => output_of(value, settings, registry, span),
                Err(error) => Err(error.clone()),
            };
            let eval = result.as_ref().ok().cloned();
            variables.insert(name.clone(), result);
            match output {
                Ok(output) => (LineOutput::Value(output), eval),
                Err(error) => (LineOutput::Error(error), eval),
            }
        }
    }
}

fn eval_expr(
    expr: &Expr,
    variables: &HashMap<String, Result<Eval, EvalError>>,
    settings: &Settings,
    registry: &Registry,
    span: Span,
) -> Result<Eval, EvalError> {
    match expr {
        Expr::Literal(value) => Ok(Eval::Scalar(value.clone())),
        Expr::Variable(name) => match variables.get(name) {
            Some(Ok(value)) => Ok(value.clone()),
            Some(Err(error)) => Err(EvalError::new(span, error.message.clone())),
            None => Err(EvalError::new(span, format!("unknown variable `{name}`"))),
        },
        Expr::Belt {
            count,
            tier,
            stacked,
            item,
        } => {
            let stack_size = registry
                .entry_by_index(*item)
                .item
                .as_ref()
                .map_or(1, |item| item.stack_size);
            Ok(Eval::Scalar(belt_value(
                *count,
                *tier,
                *stacked,
                *item,
                stack_size,
                settings.belt_stacking,
            )))
        }
        Expr::Machine(spec) => eval_machine_spec(spec, registry, span),
        Expr::Neg(inner) => match eval_expr(inner, variables, settings, registry, span)? {
            Eval::Scalar(value) => Ok(Eval::Scalar(value.neg())),
            other => Err(EvalError::new(
                span,
                format!("cannot negate {}", describe(&other)),
            )),
        },
        Expr::Binary { op, lhs, rhs } => {
            let lhs = eval_expr(lhs, variables, settings, registry, span)?;
            let rhs = eval_expr(rhs, variables, settings, registry, span)?;
            eval_binary(*op, lhs, rhs, span)
        }
        Expr::In { quantity, machine } => {
            let quantity = eval_expr(quantity, variables, settings, registry, span)?;
            let machine = eval_expr(machine, variables, settings, registry, span)?;
            eval_in(quantity, machine, settings, registry, span)
        }
    }
}

fn eval_machine_spec(
    spec: &MachineSpec,
    registry: &Registry,
    span: Span,
) -> Result<Eval, EvalError> {
    let modules = merge_modules(&spec.modules);
    let recipe = spec.recipe.map(|id| registry.recipe(id));
    config::validate(registry, spec.machine, &modules, recipe, span)?;

    match spec.recipe {
        Some(recipe) => Ok(Eval::Recipe(RecipeValue {
            machine: spec.machine,
            modules,
            recipe,
            quantity: RecipeQuantity::Machines(spec.count),
        })),
        None => Ok(Eval::Machine(MachineValue {
            machine: spec.machine,
            modules,
            count: spec.count,
        })),
    }
}

fn merge_modules(modules: &[ModuleUse]) -> Vec<ModuleCount> {
    let mut merged: BTreeMap<EntryId, f64> = BTreeMap::new();
    for module in modules {
        *merged.entry(module.module).or_insert(0.0) += module.count;
    }
    merged
        .into_iter()
        .map(|(module, count)| ModuleCount { module, count })
        .collect()
}

fn eval_in(
    quantity: Eval,
    machine: Eval,
    settings: &Settings,
    registry: &Registry,
    span: Span,
) -> Result<Eval, EvalError> {
    let Eval::Scalar(value) = quantity else {
        return Err(EvalError::new(span, "`in` needs a scalar quantity"));
    };
    let Some(selection) = selection(&value) else {
        return Err(EvalError::new(
            span,
            "`in` needs an item, fluid, or craft quantity",
        ));
    };

    let (machine_id, modules, recipe) = match machine {
        Eval::Machine(machine) => {
            if machine.count != 1.0 {
                return Err(EvalError::new(
                    span,
                    "`in` needs exactly one machine; scale the result instead",
                ));
            }
            (machine.machine, machine.modules, None)
        }
        Eval::Recipe(recipe) => {
            if recipe.quantity != RecipeQuantity::Machines(1.0) {
                return Err(EvalError::new(
                    span,
                    "`in` needs exactly one machine; scale the result instead",
                ));
            }
            (recipe.machine, recipe.modules, Some(recipe.recipe))
        }
        Eval::Scalar(_) => {
            return Err(EvalError::new(span, "`in` needs a machine on the right"));
        }
    };

    let recipe = match recipe {
        Some(recipe) => recipe,
        None => {
            if selection.unit == BaseUnit::Craft {
                return Err(EvalError::new(
                    span,
                    "a craft quantity needs an explicit `making` recipe",
                ));
            }
            config::resolve_recipe(registry, machine_id, selection.unit, span)?
        }
    };
    let recipe_data = registry.recipe(recipe);
    config::validate(registry, machine_id, &modules, Some(recipe_data), span)?;

    let net = config::coefficients(registry, machine_id, &modules, recipe_data, settings, span)?;
    let coefficient = net.get(&selection.unit).copied().unwrap_or(0.0);
    let divisor = if selection.item_or_fluid {
        coefficient.abs()
    } else {
        coefficient
    };
    let magnitude = value.coef() / divisor;
    let quantity = if selection.rate {
        RecipeQuantity::Machines(magnitude)
    } else {
        RecipeQuantity::MachineSeconds(magnitude)
    };

    Ok(Eval::Recipe(RecipeValue {
        machine: machine_id,
        modules,
        recipe,
        quantity,
    }))
}

#[derive(Debug, Clone, Copy)]
struct Selection {
    unit: BaseUnit,
    /// True for a per-second quantity, false for a total.
    rate: bool,
    item_or_fluid: bool,
}

fn selection(value: &Value) -> Option<Selection> {
    let units = value.units();
    let (unit, rate) = match units.len() {
        1 => {
            let (&unit, &exponent) = units.iter().next()?;
            if exponent != 1 {
                return None;
            }
            (unit, false)
        }
        2 => {
            if value.exponent(BaseUnit::Second) != -1 {
                return None;
            }
            let mut found = None;
            for (&unit, &exponent) in units {
                if unit == BaseUnit::Second {
                    continue;
                }
                if exponent != 1 || found.is_some() {
                    return None;
                }
                found = Some(unit);
            }
            (found?, true)
        }
        _ => return None,
    };
    let item_or_fluid = matches!(unit, BaseUnit::Item(_) | BaseUnit::Fluid(_));
    if !item_or_fluid && unit != BaseUnit::Craft {
        return None;
    }
    Some(Selection {
        unit,
        rate,
        item_or_fluid,
    })
}

fn eval_binary(op: BinaryOp, lhs: Eval, rhs: Eval, span: Span) -> Result<Eval, EvalError> {
    match op {
        BinaryOp::Pow => match (lhs, rhs) {
            (Eval::Scalar(lhs), Eval::Scalar(rhs)) => lhs
                .pow(&rhs)
                .map(Eval::Scalar)
                .map_err(|error| value_error(error, span)),
            (lhs, rhs) => Err(EvalError::new(
                span,
                format!("cannot raise {} to {}", describe(&lhs), describe(&rhs)),
            )),
        },
        BinaryOp::Add | BinaryOp::Sub => {
            let subtract = op == BinaryOp::Sub;
            match (lhs, rhs) {
                (Eval::Scalar(lhs), Eval::Scalar(rhs)) => {
                    let result = if subtract {
                        lhs.sub(&rhs)
                    } else {
                        lhs.add(&rhs)
                    };
                    result
                        .map(Eval::Scalar)
                        .map_err(|error| value_error(error, span))
                }
                (Eval::Machine(lhs), Eval::Machine(rhs)) => {
                    same_machine(&lhs, &rhs, span)?;
                    let count = if subtract {
                        lhs.count - rhs.count
                    } else {
                        lhs.count + rhs.count
                    };
                    Ok(Eval::Machine(MachineValue {
                        machine: lhs.machine,
                        modules: lhs.modules,
                        count,
                    }))
                }
                (Eval::Recipe(lhs), Eval::Recipe(rhs)) => {
                    if lhs.machine != rhs.machine || lhs.modules != rhs.modules {
                        return Err(EvalError::new(
                            span,
                            "cannot add machines with different configurations",
                        ));
                    }
                    if lhs.recipe != rhs.recipe {
                        return Err(EvalError::new(
                            span,
                            "cannot add recipes for different outputs",
                        ));
                    }
                    same_quantity_kind(&lhs.quantity, &rhs.quantity, span)?;
                    let magnitude = if subtract {
                        lhs.quantity.magnitude() - rhs.quantity.magnitude()
                    } else {
                        lhs.quantity.magnitude() + rhs.quantity.magnitude()
                    };
                    Ok(Eval::Recipe(RecipeValue {
                        machine: lhs.machine,
                        modules: lhs.modules,
                        recipe: lhs.recipe,
                        quantity: with_magnitude(lhs.quantity, magnitude),
                    }))
                }
                (lhs, rhs) => Err(EvalError::new(
                    span,
                    format!("cannot add {} and {}", describe(&lhs), describe(&rhs)),
                )),
            }
        }
        BinaryOp::Mul => match (lhs, rhs) {
            (Eval::Scalar(lhs), Eval::Scalar(rhs)) => Ok(Eval::Scalar(lhs.mul(&rhs))),
            (Eval::Scalar(scalar), Eval::Machine(machine))
            | (Eval::Machine(machine), Eval::Scalar(scalar)) => {
                scale_machine(machine, &scalar, span)
            }
            (Eval::Scalar(scalar), Eval::Recipe(recipe))
            | (Eval::Recipe(recipe), Eval::Scalar(scalar)) => {
                multiply_recipe(recipe, &scalar, span)
            }
            (lhs, rhs) => Err(EvalError::new(
                span,
                format!("cannot multiply {} by {}", describe(&lhs), describe(&rhs)),
            )),
        },
        BinaryOp::Div => match (lhs, rhs) {
            (Eval::Scalar(lhs), Eval::Scalar(rhs)) => Ok(Eval::Scalar(lhs.div(&rhs))),
            (Eval::Machine(machine), Eval::Scalar(scalar)) => {
                if !scalar.is_dimensionless() {
                    return Err(EvalError::new(
                        span,
                        "machines can only be divided by a dimensionless value",
                    ));
                }
                Ok(Eval::Machine(MachineValue {
                    machine: machine.machine,
                    modules: machine.modules,
                    count: machine.count / scalar.coef(),
                }))
            }
            (Eval::Recipe(recipe), Eval::Scalar(scalar)) => divide_recipe(recipe, &scalar, span),
            (lhs, rhs) => Err(EvalError::new(
                span,
                format!("cannot divide {} by {}", describe(&lhs), describe(&rhs)),
            )),
        },
    }
}

fn scale_machine(machine: MachineValue, scalar: &Value, span: Span) -> Result<Eval, EvalError> {
    if !scalar.is_dimensionless() {
        return Err(EvalError::new(
            span,
            "machines can only be scaled by a dimensionless value",
        ));
    }
    Ok(Eval::Machine(MachineValue {
        machine: machine.machine,
        modules: machine.modules,
        count: machine.count * scalar.coef(),
    }))
}

fn multiply_recipe(recipe: RecipeValue, scalar: &Value, span: Span) -> Result<Eval, EvalError> {
    if scalar.is_dimensionless() {
        return Ok(Eval::Recipe(RecipeValue {
            quantity: with_magnitude(recipe.quantity, recipe.quantity.magnitude() * scalar.coef()),
            ..recipe
        }));
    }
    if !is_time(scalar) {
        return Err(EvalError::new(
            span,
            "recipes can only be scaled by a dimensionless value or a time",
        ));
    }
    match recipe.quantity {
        RecipeQuantity::Machines(count) => Ok(Eval::Recipe(RecipeValue {
            quantity: RecipeQuantity::MachineSeconds(count * scalar.coef()),
            ..recipe
        })),
        RecipeQuantity::MachineSeconds(_) => Err(EvalError::new(
            span,
            "cannot multiply machine time by a time",
        )),
    }
}

fn divide_recipe(recipe: RecipeValue, scalar: &Value, span: Span) -> Result<Eval, EvalError> {
    if scalar.is_dimensionless() {
        return Ok(Eval::Recipe(RecipeValue {
            quantity: with_magnitude(recipe.quantity, recipe.quantity.magnitude() / scalar.coef()),
            ..recipe
        }));
    }
    if !is_time(scalar) {
        return Err(EvalError::new(
            span,
            "recipes can only be divided by a dimensionless value or a time",
        ));
    }
    match recipe.quantity {
        RecipeQuantity::MachineSeconds(tau) => Ok(Eval::Recipe(RecipeValue {
            quantity: RecipeQuantity::Machines(tau / scalar.coef()),
            ..recipe
        })),
        RecipeQuantity::Machines(_) => Err(EvalError::new(
            span,
            "cannot divide a machine count by a time",
        )),
    }
}

fn same_machine(lhs: &MachineValue, rhs: &MachineValue, span: Span) -> Result<(), EvalError> {
    if lhs.machine != rhs.machine || lhs.modules != rhs.modules {
        return Err(EvalError::new(
            span,
            "cannot add machines with different configurations",
        ));
    }
    Ok(())
}

fn same_quantity_kind(
    lhs: &RecipeQuantity,
    rhs: &RecipeQuantity,
    span: Span,
) -> Result<(), EvalError> {
    match (lhs, rhs) {
        (RecipeQuantity::Machines(_), RecipeQuantity::Machines(_))
        | (RecipeQuantity::MachineSeconds(_), RecipeQuantity::MachineSeconds(_)) => Ok(()),
        _ => Err(EvalError::new(span, "cannot add machines to machine time")),
    }
}

fn with_magnitude(quantity: RecipeQuantity, magnitude: f64) -> RecipeQuantity {
    match quantity {
        RecipeQuantity::Machines(_) => RecipeQuantity::Machines(magnitude),
        RecipeQuantity::MachineSeconds(_) => RecipeQuantity::MachineSeconds(magnitude),
    }
}

fn is_time(value: &Value) -> bool {
    value.units().len() == 1 && value.exponent(BaseUnit::Second) == 1
}

fn output_of(
    value: &Eval,
    settings: &Settings,
    registry: &Registry,
    span: Span,
) -> Result<Output, EvalError> {
    match value {
        Eval::Scalar(value) => Ok(Output::Scalar(value.clone())),
        Eval::Machine(machine) => Ok(Output::Machine(MachineOutput {
            machine: machine.machine,
            count: machine.count,
        })),
        Eval::Recipe(recipe) => {
            let recipe_data = registry.recipe(recipe.recipe);
            let net = config::coefficients(
                registry,
                recipe.machine,
                &recipe.modules,
                recipe_data,
                settings,
                span,
            )?;
            let multiplier = recipe.quantity.magnitude();
            let mut scaled = BTreeMap::new();
            for (unit, amount) in net {
                let amount = amount * multiplier;
                if amount != 0.0 {
                    scaled.insert(unit, amount);
                }
            }
            let modifiers = config::modifiers(
                registry,
                recipe.machine,
                &recipe.modules,
                Some(recipe_data),
                settings,
            );
            let total_energy_j = match recipe.quantity {
                RecipeQuantity::MachineSeconds(tau) => Some(tau * modifiers.power_w),
                RecipeQuantity::Machines(_) => None,
            };
            Ok(Output::Recipe(RecipeOutput {
                machine: recipe.machine,
                quantity: recipe.quantity,
                net: scaled,
                per_machine_power_w: modifiers.power_w,
                total_energy_j,
            }))
        }
    }
}

fn describe(value: &Eval) -> &'static str {
    match value {
        Eval::Scalar(_) => "a value",
        Eval::Machine(_) => "a machine",
        Eval::Recipe(_) => "a recipe",
    }
}

fn value_error(error: ValueError, span: Span) -> EvalError {
    EvalError::new(span, error.to_string())
}

/// Hash of a source line, used to detect stale outputs.
pub fn line_hash(line: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    line.hash(&mut hasher);
    hasher.finish()
}
