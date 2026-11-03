//! Evaluation: statements, the environment, loops, and cancellation
//! (§2.3–§2.7, §5).

use std::collections::BTreeMap;

use hactorio_data::{EntryId, PrototypeKind, RecipeId, Registry};

use super::parse::{
    self, AssignOp, BinaryOp, Expr, Line, ParseError, Place, Property, Selector, Span,
};
use super::process::{self, Rows};
use super::value::{
    Amount, Config, Extent, Item, Machine, MultiValue, Process, Quality, Quantity, RecipeRef,
    Resource, ResourceRef, RowKey, StackLimit, Stacks, Value, ValueError,
};
use crate::Settings;

/// One line's result (§2.5).
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Blank,
    Value(Value),
    /// An error with the span it should be underlined at.
    Error { message: String, span: Span },
    None,
}

impl Output {
    pub fn error(&self) -> Option<(&str, Span)> {
        match self {
            Output::Error { message, span } => Some((message, *span)),
            _ => None,
        }
    }
}

impl Evaluation {
    /// No document has been evaluated yet.
    pub fn empty() -> Self {
        Self {
            outputs: Vec::new(),
            hashes: Vec::new(),
            variables: Vec::new(),
            cancelled: false,
        }
    }
}

/// The result of evaluating a document.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub outputs: Vec<Output>,
    /// Hash of each source line, so a stale result can be recognised.
    pub hashes: Vec<u64>,
    /// The names in scope *before* each line, sorted.
    pub variables: Vec<Vec<String>>,
    pub cancelled: bool,
}

struct Cancelled;

type Env = BTreeMap<String, Result<Value, ValueError>>;

/// Evaluates a document. Evaluation stops without producing output when
/// `cancel` returns true, and the check runs at every statement boundary
/// (§2.7).
pub fn evaluate(
    document: &str,
    registry: &Registry,
    settings: &Settings,
    cancel: &dyn Fn() -> bool,
) -> Evaluation {
    let lines: Vec<(usize, Result<Line, ParseError>)> = document
        .lines()
        .enumerate()
        .map(|(index, line)| (index, parse::parse_line(line, registry)))
        .collect();

    let lengths: Vec<usize> = document.lines().map(str::len).collect();
    let hashes: Vec<u64> = document.lines().map(line_hash).collect();
    let mut runner = Runner {
        lines: &lines,
        outputs: vec![Output::Blank; lines.len()],
        lengths,
        hashes: hashes.clone(),
        variables: vec![Vec::new(); lines.len()],
        env: Env::new(),
        registry,
        settings,
        cancel,
    };

    if runner.run(0, lines.len()).is_err() {
        return Evaluation {
            outputs: Vec::new(),
            hashes: Vec::new(),
            variables: Vec::new(),
            cancelled: true,
        };
    }

    Evaluation {
        outputs: runner.outputs,
        hashes: runner.hashes,
        variables: runner.variables,
        cancelled: false,
    }
}

struct Runner<'a> {
    lines: &'a [(usize, Result<Line, ParseError>)],
    outputs: Vec<Output>,
    lengths: Vec<usize>,
    hashes: Vec<u64>,
    variables: Vec<Vec<String>>,
    env: Env,
    registry: &'a Registry,
    settings: &'a Settings,
    cancel: &'a dyn Fn() -> bool,
}

/// Hash of a source line, so a stale result can be recognised.
pub fn line_hash(line: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in line.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl Runner<'_> {
    fn run(&mut self, start: usize, end: usize) -> Result<(), Cancelled> {
        let mut index = start;
        while index < end {
            if (self.cancel)() {
                return Err(Cancelled);
            }
            let (line, parsed) = &self.lines[index];
            let line = *line;
            self.variables[line] = self.env.keys().cloned().collect();
            match parsed {
                Err(error) => {
                self.outputs[line] = Output::Error {
                    message: error.message.clone(),
                    span: error.span,
                };
            }
                Ok(Line::Blank | Line::Comment) => self.outputs[line] = Output::Blank,
                Ok(Line::Expression(expr)) => {
                    self.outputs[line] = match self.eval(expr) {
                        Ok(value) => Output::Value(value),
                        Err(error) => Output::Error {
                            message: error.to_string(),
                            span: self.line_span(line),
                        },
                    };
                }
                Ok(Line::Definition { place, op, value }) => {
                    self.outputs[line] = match self.define(place, *op, value) {
                        Ok(value) => Output::Value(value),
                        Err(error) => Output::Error {
                            message: error.to_string(),
                            span: self.line_span(line),
                        },
                    };
                }
                Ok(Line::LoopStart(count)) => {
                    let Some(close) = self.find_loop_end(index + 1, end) else {
                        self.outputs[line] = Output::Error {
                            message: "`loop` is never closed".to_string(),
                            span: self.line_span(line),
                        };
                        index += 1;
                        continue;
                    };
                    let iterations = match self.eval(count) {
                        Ok(value) => match iterations(&value) {
                            Ok(iterations) => iterations,
                            Err(error) => {
                                self.outputs[line] = Output::Error {
                                    message: error.to_string(),
                                    span: self.line_span(line),
                                };
                                index = close + 1;
                                continue;
                            }
                        },
                        Err(error) => {
                            self.outputs[line] = Output::Error {
                                message: error.to_string(),
                                span: self.line_span(line),
                            };
                            index = close + 1;
                            continue;
                        }
                    };
                    self.outputs[line] = Output::None;
                    self.outputs[self.lines[close].0] = Output::None;
                    for _ in 0..iterations {
                        self.run(index + 1, close)?;
                    }
                    index = close + 1;
                    continue;
                }
                Ok(Line::LoopEnd) => {
                    self.outputs[line] = Output::Error {
                        message: "`}` without `loop`".to_string(),
                        span: self.line_span(line),
                    };
                }
            }
            index += 1;
        }
        Ok(())
    }

    /// The index of the `}` matching the `loop {` at `start - 1`.
    fn find_loop_end(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = 1;
        for index in start..end {
            match &self.lines[index].1 {
                Ok(Line::LoopStart(_)) => depth += 1,
                Ok(Line::LoopEnd) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn line_span(&self, line: usize) -> Span {
        Span::new(0, self.lengths.get(line).copied().unwrap_or(0))
    }

    fn lookup(&self, name: &str) -> Result<Value, ValueError> {
        match self.env.get(name) {
            Some(Ok(value)) => Ok(value.clone()),
            Some(Err(error)) => Err(error.clone()),
            None => Err(ValueError::new(format!("`{name}` is not defined"))),
        }
    }

    fn define(&mut self, place: &Place, op: AssignOp, value: &Expr) -> Result<Value, ValueError> {
        let result = match &place.selector {
            // Only a process's resource rows are assignable (§2.2).
            Some(Selector::Resource(resource)) => self
                .lookup(&place.name)
                .and_then(|current| Ok((current, self.eval(value)?)))
                .and_then(|(current, rhs)| {
                    assign_resource(&current, resource, op, &rhs, self.registry, self.settings)
                }),
            Some(other) => Err(ValueError::new(format!(
                "`{}` is not assignable; only resources are",
                describe_selector(*other)
            ))),
            None => match op.binary() {
                None => self.eval(value),
                Some(binary) => self
                    .lookup(&place.name)
                    .and_then(|current| Ok((current, self.eval(value)?)))
                    .and_then(|(current, rhs)| {
                        binary_values(binary, &current, &rhs, self.registry, self.settings)
                    }),
            },
        };
        self.env.insert(place.name.clone(), result.clone());
        result
    }

    fn eval(&self, expr: &Expr) -> Result<Value, ValueError> {
        match expr {
            Expr::Number(value) => Ok(Value::number(*value)),
            Expr::Unit(unit) => Ok(Value::quantity(unit_quantity(*unit))),
            Expr::Token(id) => self.token(*id, None),
            Expr::Qualified { id, quality } => self.token(*id, Some(*quality)),
            Expr::Quality(quality) => Ok(Value::Quality(*quality)),
            Expr::Variable(name) => self.lookup(name),
            Expr::Negate(inner) => self.eval(inner)?.negate(),
            Expr::StackSize(inner) => self.stack_size(&self.eval(inner)?),
            Expr::StackedBelt(inner) => {
                let value = self.eval(inner)?;
                match value {
                    Value::Multi(multi) => Ok(Value::Multi(
                        multi.stacked(belt_stacking_level(self.settings, self.registry))?,
                    )),
                    Value::Process(_) => Err(ValueError::new(
                        "`:stacked-belt:` needs a value, not a process",
                    )),
                    Value::Quality(_) => {
                        Err(ValueError::new("`:stacked-belt:` needs a value, not a quality"))
                    }
                }
            }
            Expr::Recycling(inner) => self.recycling(&self.eval(inner)?),
            Expr::Select { value, selector } => {
                let value = self.eval(value)?;
                self.select(&value, *selector)
            }
            Expr::SelectBy { value, selector } => {
                let value = self.eval(value)?;
                let reference = match self.eval(selector)? {
                    Value::Multi(multi) => multi
                        .resource
                        .ok_or_else(|| ValueError::new("a selector needs a resource"))?,
                    _ => return Err(ValueError::new("a selector needs a resource")),
                };
                self.select(&value, Selector::Resource(reference))
            }
            Expr::Binary { op, lhs, rhs } => {
                let left = self.eval(lhs)?;
                let right = self.eval(rhs)?;
                binary_values(*op, &left, &right, self.registry, self.settings)
            }
            Expr::Juxtapose(operands) => {
                let mut values = Vec::with_capacity(operands.len());
                for operand in operands {
                    values.push(self.eval(operand)?);
                }
                let stack_size = |resource: Resource| match resource {
                    Resource::Item(item) => self
                        .registry
                        .entry_by_index(item.id)
                        .item
                        .as_ref()
                        .map(|item| item.stack_size),
                    Resource::Fluid(_) => None,
                };
                let validate_config = |multi: &MultiValue| {
                    if let Some(config) = &multi.config {
                        validate_config(config, self.registry)?;
                    }
                    if let Some(machine) = &multi.machine {
                        validate_machine(machine, self.registry)?;
                    }
                    Ok(())
                };
                let mut iter = values.into_iter().rev();
                let mut acc = iter.next().expect("a chain has at least one operand");
                for value in iter {
                    acc = value.juxtapose(&acc, &stack_size, &validate_config)?;
                }
                Ok(acc)
            }
        }
    }

    /// A prototype token as a value (§3.3, §6.5).
    fn token(&self, id: EntryId, quality: Option<Quality>) -> Result<Value, ValueError> {
        let entry = self.registry.entry_by_index(id);
        let concrete = quality.unwrap_or(Quality::Normal);
        let mut multi = MultiValue::default();

        if entry.item.is_some() {
            multi.quantity = Some(Quantity::resource(Resource::Item(Item {
                id,
                quality: concrete,
            })));
            multi.resource = Some(ResourceRef {
                id,
                is_fluid: false,
                quality,
            });
        } else if entry.fluid {
            multi.quantity = Some(Quantity::resource(Resource::Fluid(id)));
            multi.resource = Some(ResourceRef {
                id,
                is_fluid: true,
                quality: None,
            });
        }
        if let Some(item) = &entry.item {
            if let Some(belt) = item.belt {
                // A belt moves `speed * 60` tiles per second, each holding 8 items.
                let cap = StackLimit::Limit(1);
                multi.stack_rate = Some(Stacks::items(cap, belt.speed * 60.0 * 8.0));
                multi.stack_capacity = Some(Stacks::items(cap, 8.0));
            }
            if let Some(inserter) = item.inserter {
                let capacity = inserter_capacity(inserter, self.settings, self.registry);
                let cycles = inserter_cycles(inserter).ok_or_else(|| {
                    ValueError::new(format!(
                        "`:{}:` has an unusable rotation speed",
                        entry.id
                    ))
                })?;
                multi.stack_rate = Some(Stacks::items(StackLimit::Limit(capacity.max(1)), cycles));
            }
            if is_barrel(&entry.id, item.subgroup.as_deref()) {
                // The barreling recipe fills one barrel with 50 fluid. The
                // prototype does not say so, so this is the game's constant.
                multi.stack_capacity = Some(Stacks::Fluid(50.0));
            }
            if let Some(container) = item.container {
                multi.stack_capacity = if container.fluid_capacity > 0.0
                    && !super::value::is_zero(container.fluid_capacity)
                {
                    Some(Stacks::Fluid(container.fluid_capacity))
                } else if container.slots > 0 {
                    Some(Stacks::items(StackLimit::Full, f64::from(container.slots)))
                } else {
                    None
                };
            }
            if let Some(pump) = item.pump {
                multi.stack_rate = Some(Stacks::Fluid(pump.pumping_speed * 60.0));
            }
        }
        if entry.module.is_some() {
            multi.config = Some(Config::module(id, concrete));
        }
        if entry.beacon.is_some() {
            multi.config = Some(Config::beacon(id, concrete));
        }
        if entry.item.as_ref().is_some_and(|item| item.machine.is_some()) {
            multi.machine = Some(Machine::new(id, concrete));
        }
        if let Some(recipe) = entry.recipe {
            multi.recipe = Some(RecipeRef {
                id: recipe,
                quality,
            });
        }

        if multi.quantity.is_none() && multi.resource.is_none() && multi.recipe.is_none() {
            return Err(ValueError::new(format!("`:{}:` has no usable facets", entry.id)));
        }
        Ok(Value::Multi(multi))
    }

    fn stack_size(&self, value: &Value) -> Result<Value, ValueError> {
        let Value::Multi(multi) = value else {
            return Err(ValueError::new("`.:stack-size:` needs an item, not a quality"));
        };
        let Some(resource) = multi.resource else {
            return Err(ValueError::new(format!(
                "`.:stack-size:` needs a resource facet ({})",
                multi.facets()
            )));
        };
        if resource.is_fluid {
            return Err(ValueError::new("a fluid has no stack size"));
        }
        let entry = self.registry.entry_by_index(resource.id);
        let Some(item) = &entry.item else {
            return Err(ValueError::new(format!(
                "`:{}:` is not an item",
                entry.id
            )));
        };
        Ok(Value::number(f64::from(item.stack_size)))
    }

    fn recycling(&self, value: &Value) -> Result<Value, ValueError> {
        let Value::Multi(multi) = value else {
            return Err(ValueError::new("`:recycling:` needs an item, not a quality"));
        };
        let Some(resource) = multi.resource else {
            return Err(ValueError::new(format!(
                "`:recycling:` needs a resource facet ({})",
                multi.facets()
            )));
        };
        if resource.is_fluid {
            return Err(ValueError::new("a fluid cannot be recycled"));
        }
        let entry = self.registry.entry_by_index(resource.id);
        let recipe = self
            .registry
            .recipes()
            .iter()
            .position(|recipe| {
                recipe.categories.iter().any(|category| category.as_ref() == "recycling")
                    && recipe
                        .ingredients
                        .iter()
                        .any(|ingredient| ingredient.name.as_ref() == entry.id.as_ref())
            })
            .map(RecipeId::from_index);
        let Some(id) = recipe else {
            return Err(ValueError::new(format!(
                "no recycling recipe for `:{}:`",
                entry.id
            )));
        };
        Ok(Value::Multi(MultiValue {
            recipe: Some(RecipeRef {
                id,
                quality: resource.quality,
            }),
            ..MultiValue::default()
        }))
    }

    fn select(&self, value: &Value, selector: Selector) -> Result<Value, ValueError> {
        // Quality properties read a quality operand, not facets.
        match selector {
            Selector::Property(Property::Level) => {
                return Ok(Value::number(f64::from(quality_of(value)?.level())));
            }
            Selector::Property(Property::Multiplier) => {
                return Ok(Value::number(quality_of(value)?.multiplier()));
            }
            _ => {}
        }
        if let Value::Process(process) = value {
            return self.select_process(process, selector);
        }
        let Value::Multi(multi) = value else {
            return Err(ValueError::new("`.` needs a value, not a quality"));
        };
        match selector {
            Selector::Resource(resource) => Ok(Value::Multi(multi.select(&resource)?)),
            Selector::Signal(super::lex::Signal::Item) => {
                Ok(Value::Multi(multi.select_medium(true)?))
            }
            Selector::Signal(super::lex::Signal::Fluid) => {
                Ok(Value::Multi(multi.select_medium(false)?))
            }
            Selector::Signal(super::lex::Signal::StackSize) => {
                self.stack_size(&Value::Multi(multi.clone()))
            }
            Selector::Property(Property::Machines) => match &multi.machine {
                Some(machine) => Ok(Value::number(machine.count())),
                None => Err(ValueError::new(format!(
                    "`.machines` needs a machine facet ({})",
                    multi.facets()
                ))),
            },
            Selector::Property(Property::Rate) => match &multi.stack_rate {
                Some(stacks) => Ok(Value::Multi(MultiValue {
                    stack_rate: Some(stacks.clone()),
                    ..MultiValue::default()
                })),
                None => Err(ValueError::new(format!(
                    "`.rate` needs a stack rate facet ({})",
                    multi.facets()
                ))),
            },
            Selector::Property(Property::Capacity) => match &multi.stack_capacity {
                Some(stacks) => Ok(Value::Multi(MultiValue {
                    stack_capacity: Some(stacks.clone()),
                    ..MultiValue::default()
                })),
                None => Err(ValueError::new(format!(
                    "`.capacity` needs a stack capacity facet ({})",
                    multi.facets()
                ))),
            },
            Selector::Property(Property::Power) => {
                let Some(machine) = &multi.machine else {
                    return Err(ValueError::new(format!(
                        "`.power` needs a machine facet ({})",
                        multi.facets()
                    )));
                };
                let modifiers = process::machine_modifiers(machine, self.registry, self.settings)
                    .ok_or_else(|| ValueError::new("that machine has no modifiers"))?;
                Ok(Value::quantity(Quantity {
                    joule: 1,
                    second: -1,
                    ..Quantity::number(modifiers.power_w * machine.count())
                }))
            }
            Selector::Property(Property::Productivity) => {
                let Some(machine) = &multi.machine else {
                    return Err(ValueError::new(format!(
                        "`.productivity` needs a machine facet ({})",
                        multi.facets()
                    )));
                };
                let modifiers = process::machine_modifiers(machine, self.registry, self.settings)
                    .ok_or_else(|| ValueError::new("that machine has no modifiers"))?;
                Ok(Value::number(1.0 + modifiers.productivity))
            }
            Selector::Signal(super::lex::Signal::Speed) => {
                let Some(machine) = &multi.machine else {
                    return Err(ValueError::new(format!(
                        "`.:speed:` needs a machine facet ({})",
                        multi.facets()
                    )));
                };
                let modifiers = process::machine_modifiers(machine, self.registry, self.settings)
                    .ok_or_else(|| ValueError::new("that machine has no modifiers"))?;
                Ok(Value::number(modifiers.speed))
            }
            Selector::Signal(super::lex::Signal::ModuleSlots) => {
                let Some(machine) = &multi.machine else {
                    return Err(ValueError::new(format!(
                        "`.:module-slots:` needs a machine facet ({})",
                        multi.facets()
                    )));
                };
                let entry = self.registry.entry_by_index(machine.entry);
                let slots = entry
                    .item
                    .as_ref()
                    .and_then(|item| item.machine.as_ref())
                    .map(|prototype| prototype.module_slots)
                    .ok_or_else(|| ValueError::new(format!("`:{}:` is not a machine", entry.id)))?;
                Ok(Value::number(f64::from(slots)))
            }
            Selector::Property(Property::Abs) => {
                Ok(Value::Multi(multi.absolute()?))
            }
            other => Err(ValueError::new(format!(
                "`{}` needs a facet this value does not have ({})",
                describe_selector(other),
                multi.facets()
            ))),
        }
    }
}

impl Runner<'_> {
    fn select_process(&self, process: &Process, selector: Selector) -> Result<Value, ValueError> {
        let machine_seconds = process.extent == Extent::MachineSeconds;
        let rate = if machine_seconds { 0 } else { -1 };
        match selector {
            Selector::Property(Property::Machines) => {
                if machine_seconds {
                    return Err(ValueError::new(
                        "`.machines` needs a process sized in machines",
                    ));
                }
                Ok(Value::number(process.machines()))
            }
            Selector::Property(Property::In) => {
                self.process_totals(process, Rows::Inputs, rate)
            }
            Selector::Property(Property::Out) => {
                self.process_totals(process, Rows::Outputs, rate)
            }
            Selector::Property(Property::Net) => self.process_totals(process, Rows::Net, rate),
            Selector::Property(Property::Crafts) => {
                let crafts = process::crafts(process, self.registry, self.settings)
                    .ok_or_else(|| ValueError::new("a row has no craft rate"))?;
                Ok(Value::quantity(Quantity {
                    amount: Amount::Number(crafts),
                    per: None,
                    second: rate,
                    joule: 0,
                    craft: 1,
                }))
            }
            Selector::Property(Property::Power) => {
                if machine_seconds {
                    return Err(ValueError::new("`.power` needs a rate process"));
                }
                let watts = process::power(process, self.registry, self.settings)
                    .ok_or_else(|| ValueError::new("a row has no power"))?;
                Ok(Value::quantity(Quantity {
                    joule: 1,
                    second: -1,
                    ..Quantity::number(watts)
                }))
            }
            Selector::Property(Property::Energy) => {
                if !machine_seconds {
                    return Err(ValueError::new("`.energy` needs a machine-seconds process"));
                }
                let joules = process::power(process, self.registry, self.settings)
                    .ok_or_else(|| ValueError::new("a row has no power"))?;
                Ok(Value::quantity(Quantity {
                    joule: 1,
                    ..Quantity::number(joules)
                }))
            }
            Selector::Signal(super::lex::Signal::Item) => {
                self.process_medium(process, true, rate, false)
            }
            Selector::Signal(super::lex::Signal::Fluid) => {
                self.process_medium(process, false, rate, false)
            }
            Selector::Signal(super::lex::Signal::Speed) => {
                let speed = process::speed(process, self.registry, self.settings).ok_or_else(
                    || ValueError::new("the rows of that process have different speeds"),
                )?;
                Ok(Value::number(speed))
            }
            Selector::Signal(super::lex::Signal::Electric) => {
                if machine_seconds {
                    let joules = process::power(process, self.registry, self.settings)
                        .ok_or_else(|| ValueError::new("a row has no power"))?;
                    Ok(Value::quantity(Quantity {
                        joule: 1,
                        ..Quantity::number(joules)
                    }))
                } else {
                    let watts = process::power(process, self.registry, self.settings)
                        .ok_or_else(|| ValueError::new("a row has no power"))?;
                    Ok(Value::quantity(Quantity {
                        joule: 1,
                        second: -1,
                        ..Quantity::number(watts)
                    }))
                }
            }
            Selector::Property(Property::Pollution) => {
                let pollution = process::pollution(
                    process,
                    self.registry,
                    self.settings,
                    machine_seconds,
                )
                .ok_or_else(|| ValueError::new("a row has no pollution"))?;
                Ok(Value::number(pollution))
            }
            Selector::Property(Property::Productivity) => {
                let multiplier = process::productivity(process, self.registry, self.settings)
                    .ok_or_else(|| {
                        ValueError::new("the rows of that process have different productivities")
                    })?;
                Ok(Value::number(multiplier))
            }
            Selector::Resource(resource) => {
                let totals = process::totals(process, self.registry, self.settings, Rows::Net)
                    .ok_or_else(|| ValueError::new("a row has no rate"))?;
                let selected = Quantity {
                    amount: Amount::Resources(totals).canonicalize(),
                    per: None,
                    second: rate,
                    joule: 0,
                    craft: 0,
                }
                .select(&resource);
                Ok(Value::quantity(selected))
            }
            other => Err(ValueError::new(format!(
                "`{}` needs a facet a process does not have",
                describe_selector(other)
            ))),
        }
    }

    /// Keeps the net rows of one medium (§5.3).
    fn process_medium(
        &self,
        process: &Process,
        item: bool,
        rate: i32,
        _unused: bool,
    ) -> Result<Value, ValueError> {
        let totals = process::totals(process, self.registry, self.settings, Rows::Net)
            .ok_or_else(|| ValueError::new("a row has no rate"))?;
        let quantity = Quantity {
            amount: Amount::Resources(totals).canonicalize(),
            per: None,
            second: rate,
            joule: 0,
            craft: 0,
        };
        Ok(Value::quantity(quantity.select_medium(item)))
    }

    fn process_totals(
        &self,
        process: &Process,
        which: Rows,
        rate: i32,
    ) -> Result<Value, ValueError> {
        let totals = process::totals(process, self.registry, self.settings, which)
            .ok_or_else(|| ValueError::new("a row has no rate"))?;
        Ok(Value::quantity(Quantity {
            amount: Amount::Resources(totals).canonicalize(),
            per: None,
            second: rate,
            joule: 0,
            craft: 0,
        }))
    }
}

fn describe_selector(selector: Selector) -> String {
    match selector {
        Selector::Property(property) => format!("{property:?}").to_lowercase(),
        Selector::Signal(signal) => format!(":{}:", signal.id()),
        Selector::Resource(resource) => format!(":{}:", resource.id.index()),
    }
}

fn quality_of(value: &Value) -> Result<Quality, ValueError> {
    match value {
        Value::Quality(quality) => Ok(*quality),
        Value::Multi(multi) => Err(ValueError::new(format!(
            "expected a quality, got facets ({})",
            multi.facets()
        ))),
        Value::Process(_) => Err(ValueError::new("expected a quality, got a process")),
    }
}

fn facets(value: &Value) -> String {
    match value {
        Value::Multi(multi) => multi.facets(),
        Value::Process(process) => format!("process ({} rows)", process.rows.len()),
        Value::Quality(quality) => format!(":{}:", quality.name()),
    }
}

fn unit_quantity(unit: super::lex::Unit) -> Quantity {
    match unit {
        super::lex::Unit::Second(seconds) => Quantity::time(seconds),
        super::lex::Unit::Joule(joules) => Quantity {
            joule: 1,
            ..Quantity::number(joules)
        },
        super::lex::Unit::Watt(watts) => Quantity {
            joule: 1,
            second: -1,
            ..Quantity::number(watts)
        },
        super::lex::Unit::Craft(crafts) => Quantity {
            craft: 1,
            ..Quantity::number(crafts)
        },
    }
}

/// Whether an item is a barrel. The filled barrels carry the `barrel`
/// subgroup; the empty one is an `intermediate-product`, so the name decides.
fn is_barrel(id: &str, subgroup: Option<&str>) -> bool {
    id == "barrel" || id.ends_with("-barrel") || subgroup == Some("barrel")
}

/// The belt stacking level: the setting, clamped to the four the game has
/// (§6.1). The stacking research sets the setting.
fn belt_stacking_level(settings: &Settings, _registry: &Registry) -> u32 {
    u32::from(settings.belt_stacking).clamp(1, 4)
}

/// Effective inserter capacity, clamped to the research levels the registry has
/// (§6.1). `hand_size` is cosmetic and plays no part.
fn inserter_capacity(
    inserter: hactorio_data::Inserter,
    settings: &Settings,
    registry: &Registry,
) -> u32 {
    let research = registry.inserter_research();
    let level = settings.inserter_capacity.min(research.max_level());
    let bonus = research.bonus(level);
    let bonus = if inserter.bulk || inserter.stack_size_bonus > 0 {
        bonus.bulk
    } else {
        bonus.non_bulk
    };
    let base = if inserter.stack_size_bonus > 0 {
        2 + inserter.stack_size_bonus
    } else if inserter.bulk {
        2
    } else {
        1
    };
    base + bonus.round() as u32
}

/// One full in-and-out swing per cycle, in cycles per second.
fn inserter_cycles(inserter: hactorio_data::Inserter) -> Option<f64> {
    let rotation_speed = inserter.rotation_speed;
    if !(rotation_speed > 0.0
        && !super::value::is_zero(rotation_speed)
        && rotation_speed < 0.5)
    {
        return None;
    }
    let ticks_half = (0.5 / rotation_speed).ceil() - 1.0;
    let cycle_ticks = 2.0 * ticks_half;
    Some(60.0 / cycle_ticks)
}

fn iterations(value: &Value) -> Result<u32, ValueError> {
    let Some(count) = value.dimensionless() else {
        return Err(ValueError::new("a loop count must be a dimensionless number"));
    };
    if count < 0.0 || count.fract().abs() > super::value::APPROX_ZERO {
        return Err(ValueError::new("a loop count must be a non-negative integer"));
    }
    if count > f64::from(u32::MAX) {
        return Err(ValueError::new("loop count is too large"));
    }
    Ok(count as u32)
}

fn binary_values(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    registry: &Registry,
    settings: &Settings,
) -> Result<Value, ValueError> {
    match op {
        BinaryOp::Add => left.add(right),
        BinaryOp::Sub => left.sub(right),
        BinaryOp::Mul => left.mul(right),
        BinaryOp::Div => left.div(right),
        BinaryOp::Pow => left.pow(right),
        BinaryOp::Compare(comparison) => {
            let Value::Multi(multi) = left else {
                return Err(ValueError::new(format!(
                    "`{}` needs a quantity on the left",
                    comparison.symbol()
                )));
            };
            // The right operand says what is compared: a quality filters by
            // quality, a plain number filters by coefficient (§5.3).
            if let Some(threshold) = right.dimensionless() {
                return Ok(Value::Multi(multi.filter_amount(comparison, threshold)?));
            }
            let quality = quality_of(right)?;
            Ok(Value::Multi(multi.filter_quality(comparison, quality)?))
        }
        BinaryOp::Make => make_value(left, right, registry, settings),
        BinaryOp::In | BinaryOp::Into | BinaryOp::From => {
            size_value(op, left, right, registry, settings)
        }
    }
}

/// Writes one resource row of a process (§2.2). The resource comes from the
/// selector, an unspecified quality means normal, and the value must be a rate
/// for a machines process or an amount for a machine-seconds one.
fn assign_resource(
    current: &Value,
    resource: &ResourceRef,
    op: AssignOp,
    rhs: &Value,
    registry: &Registry,
    settings: &Settings,
) -> Result<Value, ValueError> {
    let Value::Process(process) = current else {
        return Err(ValueError::new(format!(
            "only a process has resource rows to assign (value: {})",
            facets(current)
        )));
    };
    let target = if resource.is_fluid {
        Resource::Fluid(resource.id)
    } else {
        Resource::Item(Item {
            id: resource.id,
            quality: resource.quality.unwrap_or(Quality::Normal),
        })
    };

    let Value::Multi(multi) = rhs else {
        return Err(ValueError::new("a resource row takes a quantity"));
    };
    let Some(quantity) = &multi.quantity else {
        return Err(ValueError::new("a resource row takes a quantity"));
    };
    let coefficient = match &quantity.amount {
        Amount::Number(value) => *value,
        Amount::Resources(terms) => {
            if terms.len() != 1 {
                return Err(ValueError::new(
                    "a resource row takes one resource, not a bundle",
                ));
            }
            let (assigned, coefficient) = terms.iter().next().expect("one term");
            if *assigned != target {
                return Err(ValueError::new(format!(
                    "assigning `:{}:` needs a quantity of it",
                    resource.id.index()
                )));
            }
            *coefficient
        }
    };

    let rate = !matches!(process.extent, Extent::MachineSeconds);
    match op {
        AssignOp::Mul | AssignOp::Div => {
            if quantity.second != 0 || quantity.joule != 0 || quantity.craft != 0 {
                return Err(ValueError::new("scaling a row needs a dimensionless number"));
            }
        }
        _ if rate => {
            if quantity.second != 0 && quantity.second != -1 {
                return Err(ValueError::new(
                    "a machines process takes a rate or a dimensionless amount",
                ));
            }
        }
        _ => {
            if quantity.second != 0 {
                return Err(ValueError::new(
                    "a machine-seconds process takes an amount",
                ));
            }
        }
    }

    // Assignment works on the row's total, so `=` replaces what the recipes
    // produce and the stored value is only the difference.
    let mut recipe_only = process.clone();
    recipe_only.net.clear();
    let base = process::totals(&recipe_only, registry, settings, Rows::Net)
        .and_then(|totals| totals.get(&target).copied())
        .unwrap_or(0.0);
    let total = base + process.net.get(&target).copied().unwrap_or(0.0);
    let total = match op {
        AssignOp::Assign => coefficient,
        AssignOp::Add => total + coefficient,
        AssignOp::Sub => total - coefficient,
        AssignOp::Mul => total * coefficient,
        AssignOp::Div => total / coefficient,
    };
    let assigned = total - base;
    let mut net = process.net.clone();
    if super::value::is_zero(assigned) {
        net.remove(&target);
    } else {
        net.insert(target, assigned);
    }
    Ok(Value::Process(Process { net, ..process.clone() }))
}

/// The resources a process's net rows mention, for completion ranking.
pub fn process_resources(
    process: &Process,
    registry: &Registry,
    settings: &Settings,
) -> Vec<Resource> {
    process::totals(process, registry, settings, Rows::Net)
        .map(|totals| totals.keys().copied().collect())
        .unwrap_or_default()
}

/// Whether `name` is a property this value can read (§6.6), which is what
/// completion offers after a dot.
pub fn property_is_available(value: &Value, name: &str, _registry: &Registry, _settings: &Settings) -> bool {
    match (value, name) {
        (Value::Quality(_), "level" | "multiplier") => true,
        (Value::Multi(_), "abs" | "item" | "fluid") => true,
        (Value::Multi(multi), "rate") => multi.stack_rate.is_some(),
        (Value::Multi(multi), "capacity") => multi.stack_capacity.is_some(),
        (Value::Multi(multi), "machines" | "speed" | "module-slots") => multi.machine.is_some(),
        (Value::Multi(multi), "power" | "productivity") => multi.machine.is_some(),
        (Value::Multi(multi), "stack-size") => multi.resource.is_some_and(|resource| !resource.is_fluid),
        (Value::Process(process), "machines") => process.extent != Extent::MachineSeconds,
        (Value::Process(process), "energy") => process.extent == Extent::MachineSeconds,
        (Value::Process(_),
            "in" | "out" | "net" | "crafts" | "pollution" | "productivity" | "power"
            | "electric" | "item" | "fluid" | "speed") => true,
        _ => false,
    }
}

/// A machine holds only what its slots and effects allow (§5.2, §6.3).
fn validate_machine(machine: &Machine, registry: &Registry) -> Result<(), ValueError> {
    let entry = registry.entry_by_index(machine.entry);
    let Some(prototype) = entry.item.as_ref().and_then(|item| item.machine.as_ref()) else {
        return Ok(());
    };
    let total: u32 = machine.config.modules.values().sum();
    if total > prototype.module_slots {
        return Err(ValueError::new(format!(
            "`:{}:` holds at most {} modules, not {total}",
            entry.id, prototype.module_slots
        )));
    }
    for (module, _) in machine.config.modules.keys() {
        let module_entry = registry.entry_by_index(*module);
        let Some(module) = &module_entry.module else {
            continue;
        };
        let allowed = match module.category.as_ref() {
            "speed" => prototype.allowed_effects.speed,
            "productivity" => prototype.allowed_effects.productivity,
            "efficiency" => prototype.allowed_effects.consumption,
            "quality" => prototype.allowed_effects.quality,
            _ => true,
        };
        if !allowed {
            return Err(ValueError::new(format!(
                "`:{}:` cannot hold `:{}:`",
                entry.id, module_entry.id
            )));
        }
    }
    validate_config(&machine.config, registry)
}

/// A beacon holds only modules its `allowed_effects` allow, up to its slots
/// (§6.2).
fn validate_config(config: &Config, registry: &Registry) -> Result<(), ValueError> {
    for group in config.beacons.keys() {
        let entry = registry.entry_by_index(group.beacon);
        let Some(beacon) = &entry.beacon else {
            return Err(ValueError::new(format!("`:{}:` is not a beacon", entry.id)));
        };
        let slots: u32 = group.modules.values().sum();
        if slots > beacon.module_slots {
            return Err(ValueError::new(format!(
                "`:{}:` holds at most {} modules, not {slots}",
                entry.id, beacon.module_slots
            )));
        }
        for (module, _) in group.modules.keys() {
            let module_entry = registry.entry_by_index(*module);
            let Some(module) = &module_entry.module else {
                return Err(ValueError::new(format!(
                    "`:{}:` is not a module",
                    module_entry.id
                )));
            };
            let allowed = match module.category.as_ref() {
                "speed" => beacon.allowed_effects.speed,
                "productivity" => beacon.allowed_effects.productivity,
                "efficiency" => beacon.allowed_effects.consumption,
                "quality" => beacon.allowed_effects.quality,
                _ => false,
            };
            if !allowed {
                return Err(ValueError::new(format!(
                    "`:{}:` cannot hold `:{}:`",
                    entry.id, module_entry.id
                )));
            }
        }
    }
    Ok(())
}

/// `machine making recipe` (§5.4).
fn make_value(
    left: &Value,
    right: &Value,
    registry: &Registry,
    settings: &Settings,
) -> Result<Value, ValueError> {
    let Value::Multi(left) = left else {
        return Err(ValueError::new(format!(
            "`making` needs a machine facet (left: {})",
            facets(left)
        )));
    };
    let Some(machine) = &left.machine else {
        return Err(ValueError::new(format!(
            "`making` needs a machine facet (left: {})",
            left.facets()
        )));
    };
    let Value::Multi(right) = right else {
        return Err(ValueError::new("`making` needs a recipe on the right"));
    };
    let Some(recipe_ref) = right.recipe else {
        return Err(ValueError::new(format!(
            "`making` needs a recipe facet (right: {})",
            right.facets()
        )));
    };
    build_process(machine, recipe_ref.id, recipe_ref.quality, registry, settings)
}

/// Builds the unscaled or counted process a machine and recipe imply (§5.4).
fn build_process(
    machine: &Machine,
    recipe: RecipeId,
    recipe_quality: Option<Quality>,
    registry: &Registry,
    settings: &Settings,
) -> Result<Value, ValueError> {
    let entry = registry.entry_by_index(machine.entry);
    let Some(prototype) = entry.item.as_ref().and_then(|item| item.machine.as_ref()) else {
        return Err(ValueError::new(format!("`:{}:` is not a machine", entry.id)));
    };
    let prototype_recipe = registry.recipe(recipe);
    if !prototype_recipe
        .categories
        .iter()
        .any(|category| prototype.categories.iter().any(|own| own == category))
    {
        return Err(ValueError::new(format!(
            "`:{}:` cannot make `{}`",
            entry.id, prototype_recipe.id
        )));
    }
    if !prototype_recipe.allow_productivity {
        for (module, _) in machine.config.modules.keys() {
            let module = registry.entry_by_index(*module);
            if module
                .module
                .as_ref()
                .is_some_and(|module| module.effect.productivity > 0.0)
            {
                return Err(ValueError::new(format!(
                    "`{}` does not allow productivity modules",
                    prototype_recipe.id
                )));
            }
        }
    }

    // A recipe with an item input can be made at any quality; one fed only by
    // fluids cannot, so it exists at normal quality alone (§5.4).
    let upgradable = prototype_recipe
        .ingredients
        .iter()
        .any(|ingredient| ingredient.kind == PrototypeKind::Item);
    let qualities: Vec<Quality> = match recipe_quality {
        Some(quality) => {
            if quality != Quality::Normal && !upgradable {
                return Err(ValueError::new(format!(
                    "`{}` can only be made at normal quality",
                    prototype_recipe.id
                )));
            }
            vec![quality]
        }
        None if upgradable => Quality::ALL.to_vec(),
        None => vec![Quality::Normal],
    };
    let scale = machine.count();
    let rows: BTreeMap<RowKey, f64> = qualities
        .into_iter()
        .map(|quality| {
            let key = RowKey {
                machine: machine.entry,
                machine_quality: machine.quality,
                config: machine.config.clone(),
                recipe,
                recipe_quality: quality,
            };
            (key, scale)
        })
        .collect();
    // A counted machine is already sized, and sizing picks one recipe.
    let extent = if machine.count.is_some() {
        Extent::Machines
    } else {
        Extent::Unscaled
    };
    let _ = settings;
    let process = Process::new(extent, rows);
    if extent == Extent::Machines {
        return process
            .collapsed()
            .map(Value::Process)
            .ok_or_else(|| ValueError::new("that machine makes several recipes, so name one"));
    }
    Ok(Value::Process(process))
}

/// `quantity in/from/into process` (§5.4).
fn size_value(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    registry: &Registry,
    settings: &Settings,
) -> Result<Value, ValueError> {
    let Value::Multi(left) = left else {
        return Err(ValueError::new(format!(
            "`{}` needs a quantity on the left",
            op.symbol()
        )));
    };
    let Some(quantity) = &left.quantity else {
        return Err(ValueError::new(format!(
            "`{}` needs a quantity on the left ({})",
            op.symbol(),
            left.facets()
        )));
    };
    if quantity.per.is_some() || quantity.joule != 0 {
        return Err(ValueError::new(format!(
            "`{}` needs a plain item, fluid, or craft quantity",
            op.symbol()
        )));
    }

    let is_rate = match quantity.second {
        -1 => true,
        0 => false,
        _ => {
            return Err(ValueError::new(format!(
                "`{}` needs a rate or an amount",
                op.symbol()
            )));
        }
    };

    let terms: Vec<(Resource, f64)> = match &quantity.amount {
        Amount::Resources(terms) => terms.iter().map(|(resource, coefficient)| (*resource, *coefficient)).collect(),
        Amount::Number(_) if quantity.craft > 0 => Vec::new(),
        Amount::Number(value) if super::value::is_zero(*value) => Vec::new(),
        Amount::Number(_) => {
            return Err(ValueError::new(format!(
                "`{}` needs a resource to size by",
                op.symbol()
            )));
        }
    };

    // Nothing to size is a process with no rows, and no unit either.
    if terms.is_empty() && quantity.craft == 0 {
        return Ok(Value::Process(Process::new(
            Extent::Unspecified,
            BTreeMap::new(),
        )));
    }

    let rows_wanted = which_rows(op);
    let mut rows: BTreeMap<RowKey, f64> = BTreeMap::new();

    match right {
        Value::Process(process) => {
            if process.extent != Extent::Unscaled {
                return Err(ValueError::new(
                    "these operators size a process that is not sized yet",
                ));
            }
            for (resource, amount) in &terms {
                let key = find_row(process, *resource, rows_wanted, registry, settings)?;
                if rows.contains_key(&key) {
                    return Err(ValueError::new(
                        "two resources select the same recipe, which would double count",
                    ));
                }
                let coefficient = row_coefficient(&key, *resource, rows_wanted, registry, settings)?;
                rows.insert(key, amount / coefficient);
            }
            if quantity.craft > 0 {
                let magnitude = quantity.amount.number_value().unwrap_or(0.0);
                let key = process
                    .rows
                    .keys()
                    .next()
                    .ok_or_else(|| ValueError::new("the process has no rows to craft"))?;
                let rates = process::row_rates(key, registry, settings)
                    .ok_or_else(|| ValueError::new("a row has no rate"))?;
                rows.insert(key.clone(), magnitude / rates.craft_per_second);
            }
        }
        Value::Multi(multi) => {
            let Some(machine) = &multi.machine else {
                return Err(ValueError::new(format!(
                    "`{}` needs a process or machine facet (right: {})",
                    op.symbol(),
                    multi.facets()
                )));
            };
            // Each resource infers its own recipe, so a machine that makes
            // several things sizes for all of them at once.
            for (resource, amount) in &terms {
                let recipe = infer_recipe(machine, *resource, rows_wanted, registry, settings)?;
                let key = RowKey {
                    machine: machine.entry,
                    machine_quality: machine.quality,
                    config: machine.config.clone(),
                    recipe,
                    recipe_quality: resource.quality().unwrap_or(Quality::Normal),
                };
                if rows.contains_key(&key) {
                    return Err(ValueError::new(
                        "two resources select the same recipe, which would double count",
                    ));
                }
                let coefficient = row_coefficient(&key, *resource, rows_wanted, registry, settings)?;
                rows.insert(key, amount / coefficient);
            }
            if quantity.craft > 0 {
                return Err(ValueError::new(format!(
                    "`{}` needs a resource to choose a recipe with",
                    op.symbol()
                )));
            }
        }
        _ => {
            return Err(ValueError::new(format!(
                "`{}` needs a process or machine facet (right: {})",
                op.symbol(),
                facets(right)
            )));
        }
    }

    let extent = if is_rate {
        Extent::Machines
    } else {
        Extent::MachineSeconds
    };
    Ok(Value::Process(Process::new(extent, rows)))
}

/// The coefficient a row contributes for `resource`, positive.
fn row_coefficient(
    key: &RowKey,
    resource: Resource,
    which: Rows,
    registry: &Registry,
    settings: &Settings,
) -> Result<f64, ValueError> {
    let rates =
        process::row_rates(key, registry, settings).ok_or_else(|| ValueError::new("a row has no rate"))?;
    let coefficient = match which {
        Rows::Inputs => rates.inputs.get(&resource).copied().unwrap_or(0.0),
        Rows::Outputs => rates.outputs.get(&resource).copied().unwrap_or(0.0),
        Rows::Net => {
            rates.outputs.get(&resource).copied().unwrap_or(0.0)
                - rates.inputs.get(&resource).copied().unwrap_or(0.0)
        }
    };
    if coefficient.abs() <= super::value::APPROX_ZERO {
        return Err(ValueError::new("that recipe does not involve the resource"));
    }
    Ok(coefficient.abs())
}

/// The row of `process` that involves `resource`, matching its quality.
fn find_row(
    process: &Process,
    resource: Resource,
    which: Rows,
    registry: &Registry,
    settings: &Settings,
) -> Result<RowKey, ValueError> {
    for key in process.rows.keys() {
        if key.recipe_quality != resource.quality().unwrap_or(key.recipe_quality) {
            continue;
        }
        if row_coefficient(key, resource, which, registry, settings).is_ok() {
            return Ok(key.clone());
        }
    }
    Err(ValueError::new("no row of that process involves the resource"))
}

fn which_rows(op: BinaryOp) -> Rows {
    match op {
        BinaryOp::Into => Rows::Inputs,
        BinaryOp::From => Rows::Outputs,
        _ => Rows::Net,
    }
}

/// The one recipe of a machine whose relevant rows involve `resource` (§5.4).
fn infer_recipe(
    machine: &Machine,
    resource: Resource,
    rows: Rows,
    registry: &Registry,
    settings: &Settings,
) -> Result<RecipeId, ValueError> {
    let entry = registry.entry_by_index(machine.entry);
    let Some(prototype) = entry.item.as_ref().and_then(|item| item.machine.as_ref()) else {
        return Err(ValueError::new(format!("`:{}:` is not a machine", entry.id)));
    };
    let mut candidates = Vec::new();
    for (index, recipe) in registry.recipes().iter().enumerate() {
        if !recipe
            .categories
            .iter()
            .any(|category| prototype.categories.iter().any(|own| own == category))
        {
            continue;
        }
        let key = RowKey {
            machine: machine.entry,
            machine_quality: machine.quality,
            config: machine.config.clone(),
            recipe: RecipeId::from_index(index),
            recipe_quality: resource.quality().unwrap_or(Quality::Normal),
        };
        let Some(rates) = process::row_rates(&key, registry, settings) else {
            continue;
        };
        let out = rates.outputs.get(&resource).copied().unwrap_or(0.0);
        let input = rates.inputs.get(&resource).copied().unwrap_or(0.0);
        let coefficient = match rows {
            Rows::Inputs => input,
            Rows::Outputs => out,
            Rows::Net => out - input,
        };
        if coefficient > 0.0 {
            candidates.push(RecipeId::from_index(index));
        }
    }
    match candidates.len() {
        1 => Ok(candidates[0]),
        0 => Err(ValueError::new("no recipe of that machine involves it")),
        _ => Err(ValueError::new(
            "several recipes match; name one with `making`",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_eval::{Amount, test_registry};

    fn run(document: &str) -> Vec<Output> {
        let registry = test_registry();
        let evaluation = evaluate(document, &registry, &crate::Settings::default(), &|| false);
        assert!(!evaluation.cancelled);
        evaluation.outputs
    }

    fn value(document: &str) -> Value {
        match run(document).pop().expect("an output") {
            Output::Value(value) => value,
            other => panic!("expected a value, got {other:?}"),
        }
    }

    fn number(document: &str) -> f64 {
        value(document).dimensionless().expect("dimensionless")
    }

    fn error(document: &str) -> String {
        match run(document).pop().expect("an output") {
            Output::Error { message, .. } => message,
            other => panic!("expected an error, got {other:?}"),
        }
    }

    fn quantity(document: &str) -> Quantity {
        match value(document) {
            Value::Multi(multi) => multi.quantity.expect("a quantity"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn arithmetic_follows_the_spec_grouping() {
        assert_eq!(number("2 + 3 * 4"), 14.0);
        assert_eq!(number("3 * 1 / 2"), 1.5);
        assert_eq!(number("2 ^ 3 ^ 2"), 512.0);
        assert_eq!(number("-2 ^ 2"), -4.0);
        assert_eq!(number("2 - 3 + 4"), 3.0);
    }

    #[test]
    fn unit_keywords_are_quantity_literals() {
        let quantity = quantity("3s");
        assert_eq!(quantity.second, 1);
        assert_eq!(quantity.amount, Amount::Number(3.0));
    }

    #[test]
    fn definitions_report_their_value() {
        let outputs = run("x = 2\nx * 3");
        assert_eq!(outputs[0], Output::Value(Value::number(2.0)));
        assert_eq!(outputs[1], Output::Value(Value::number(6.0)));
    }

    #[test]
    fn a_failed_definition_reports_and_poisons() {
        let outputs = run("x = :nope:\ny = x + 1\nz = 1");
        assert!(matches!(outputs[0], Output::Error { .. }));
        assert!(matches!(outputs[1], Output::Error { .. }));
        assert_eq!(outputs[2], Output::Value(Value::number(1.0)));
    }

    #[test]
    fn a_redefinition_clears_the_poison() {
        let outputs = run("x = :nope:\nx = 1\nx");
        assert!(matches!(outputs[0], Output::Error { .. }));
        assert_eq!(outputs[2], Output::Value(Value::number(1.0)));
    }

    #[test]
    fn errors_do_not_stop_later_statements() {
        let outputs = run("1 / 0 * 0\n2");
        assert!(matches!(outputs[0], Output::Value(_)));
        assert_eq!(outputs[1], Output::Value(Value::number(2.0)));
    }

    #[test]
    fn loops_mutate_the_global_environment() {
        let outputs = run("x = 0\nloop 3 {\n  x += 2\n}\nx");
        assert_eq!(outputs[4], Output::Value(Value::number(6.0)));
    }

    #[test]
    fn a_loop_body_keeps_running_after_an_error() {
        let outputs = run("x = 0\nloop 2 {\n  y = :nope:\n  x += 1\n}\nx");
        assert!(matches!(outputs[2], Output::Error { .. }));
        assert_eq!(outputs[5], Output::Value(Value::number(2.0)));
    }

    #[test]
    fn a_bad_loop_count_reports_an_error_and_runs_nothing() {
        let outputs = run("loop 1.5 {\n x = 1\n}\nx");
        assert!(matches!(outputs[0], Output::Error { .. }));
        assert!(matches!(outputs[3], Output::Error { .. }));
    }

    #[test]
    fn cancellation_produces_no_output() {
        let registry = test_registry();
        let evaluation = evaluate("1", &registry, &crate::Settings::default(), &|| true);
        assert!(evaluation.cancelled);
        assert!(evaluation.outputs.is_empty());
    }

    #[test]
    fn a_number_filter_keeps_terms_by_sign() {
        let registry = test_registry();
        let settings = crate::Settings::default();
        let render = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => crate::new_eval::format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        let plate = registry.entry_by_index(registry.entry_id("iron-plate").unwrap()).glyph;
        let cable = registry.entry_by_index(registry.entry_id("copper-plate").unwrap()).glyph;

        // A bundle split by sign.
        assert_eq!(
            render("(2:iron-plate: - 3:copper-plate:) > 0"),
            format!("2{plate}")
        );
        assert_eq!(
            render("(2:iron-plate: - 3:copper-plate:) < 0"),
            format!("-3{cable}")
        );
        // A quantity with no term of that sign is zero.
        assert_eq!(render("(2:iron-plate:) < 0"), "0");

        // The quality filter still works, because a quality is not a number.
        assert_eq!(
            render("(2:iron-plate: + 3:rare::iron-plate:) > :normal:"),
            format!(
                "3{}{plate}",
                hactorio_data::quality_by_id("rare").unwrap().glyph
            )
        );
    }

    #[test]
    fn quality_filters_select_terms() {
        let quantity = quantity("(2:iron-plate: + 3:rare::iron-plate:) >= :rare:");
        assert_eq!(
            quantity.amount,
            Amount::Resources(BTreeMap::from([(
                Resource::Item(Item {
                    id: test_registry().entry_id("iron-plate").unwrap(),
                    quality: Quality::Rare
                }),
                3.0
            )]))
        );
    }

    #[test]
    fn selection_by_quality_and_medium() {
        let selected = quantity("(1:iron-plate: + 2:rare::iron-plate:).:normal::iron-plate:");
        assert_eq!(
            selected.amount,
            Amount::Resources(BTreeMap::from([(
                Resource::Item(Item {
                    id: test_registry().entry_id("iron-plate").unwrap(),
                    quality: Quality::Normal
                }),
                1.0
            )]))
        );
        assert_eq!(number(":iron-plate: .:stack-size:"), 100.0);
    }

    #[test]
    fn stack_size_needs_a_resource_facet() {
        assert!(error("(3 :iron-plate:) .:stack-size:").contains("resource facet"));
    }

    #[test]
    fn making_reports_the_missing_facet() {
        assert!(error(":iron-plate: making :iron-plate:").contains("machine facet"));
    }
}

