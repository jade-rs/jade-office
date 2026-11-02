//! Solver's three solving methods over a model given as a function of the
//! variable cells: Simplex LP for linear models (its coefficients probed
//! from the sheet, integers by branch and bound), GRG Nonlinear for smooth
//! ones (an augmented Lagrangian over quasi-Newton steps), and Evolutionary
//! for the rest (a seeded genetic search).

/// How a constraint's left side compares with its right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    AtMost,
    Equal,
    AtLeast,
}

/// What Solver aims the objective at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Goal {
    Max,
    Min,
    Value(f64),
}

/// A model as the methods see it: `n` variables, `m` constraints, and a
/// function giving the objective and each constraint's `left - right` at a
/// point, `None` where the sheet gives no number.
pub struct Model<'a> {
    pub start: Vec<f64>,
    pub goal: Goal,
    pub relations: Vec<Relation>,
    /// Variables that must be whole numbers (`int`, `bin`, `dif`).
    pub integer: Vec<bool>,
    /// Bounds each variable has of its own (`bin`'s 0..1, `dif`'s 1..n,
    /// non-negative's 0..), as `(lower, upper)`.
    pub bounds: Vec<(f64, f64)>,
    /// Groups of variables that must all differ (`dif`).
    pub different: Vec<Vec<usize>>,
    pub evaluate: &'a mut dyn FnMut(&[f64]) -> Option<Vec<f64>>,
}

/// Solver's options that steer the methods.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Constraint precision: how far a constraint may be off and still hold.
    pub precision: f64,
    /// Most iterations (trial solutions) a method takes.
    pub iterations: u32,
    /// Most seconds a method takes.
    pub seconds: f64,
    /// Integer optimality: a whole-number solution this close (as a
    /// fraction) to the best bound is good enough.
    pub integer_tolerance: f64,
    /// Convergence: GRG stops when the objective changes less than this.
    pub convergence: f64,
    /// Seeds the Evolutionary method's random numbers.
    pub seed: u64,
    /// Population size for Evolutionary.
    pub population: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            precision: 0.000001,
            iterations: 1000,
            seconds: 100.0,
            integer_tolerance: 0.01,
            convergence: 0.0001,
            seed: 0,
            population: 100,
        }
    }
}

/// How a solve ended, as Solver's Results dialog words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Solver found a solution. All constraints and optimality conditions
    /// are satisfied.
    Optimal,
    /// Solver has converged to the current solution.
    Converged,
    /// Solver could not find a feasible solution.
    Infeasible,
    /// The objective cell values do not converge.
    Unbounded,
    /// The linearity conditions required by this LP Solver are not
    /// satisfied.
    NotLinear,
    /// Stopped at the iteration or time limit.
    Limit,
    /// A cell gave no number at a trial point.
    NotNumber,
}

/// What a method found.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    pub ending: Ending,
    pub values: Vec<f64>,
    pub objective: f64,
    pub iterations: u32,
}

impl Ending {
    pub fn key(self) -> &'static str {
        match self {
            Self::Optimal => "solver.ending.optimal",
            Self::Converged => "solver.ending.converged",
            Self::Infeasible => "solver.ending.infeasible",
            Self::Unbounded => "solver.ending.unbounded",
            Self::NotLinear => "solver.ending.linear",
            Self::Limit => "solver.ending.limit",
            Self::NotNumber => "solver.ending.number",
        }
    }

    /// Whether the values found are worth keeping.
    pub fn found(self) -> bool {
        matches!(self, Self::Optimal | Self::Converged | Self::Limit)
    }
}

/// The objective as every method minimises it.
fn aimed(goal: Goal, objective: f64) -> f64 {
    match goal {
        Goal::Max => -objective,
        Goal::Min => objective,
        Goal::Value(target) => (objective - target).powi(2),
    }
}

/// How far `values` (objective then constraints) are from holding.
fn violation(model_relations: &[Relation], values: &[f64]) -> f64 {
    model_relations
        .iter()
        .zip(&values[1..])
        .map(|(relation, gap)| match relation {
            Relation::AtMost => gap.max(0.0),
            Relation::AtLeast => (-gap).max(0.0),
            Relation::Equal => gap.abs(),
        })
        .fold(0.0, f64::max)
}

fn within(bounds: &[(f64, f64)], x: &[f64], precision: f64) -> bool {
    bounds
        .iter()
        .zip(x)
        .all(|((low, high), v)| *v >= low - precision && *v <= high + precision)
}

// ---------------------------------------------------------------- Simplex LP

/// A linear program: minimise `cost·x` subject to rows `a·x (rel) b` and
/// `lower <= x <= upper`.
#[derive(Debug, Clone)]
struct Linear {
    cost: Vec<f64>,
    rows: Vec<(Vec<f64>, Relation, f64)>,
    bounds: Vec<(f64, f64)>,
}

/// The model's coefficients, probed at its start and at a step from it in
/// each variable, checked at a second point to be linear.
fn linearise(model: &mut Model) -> Result<Linear, Ending> {
    let n = model.start.len();
    let base_point: Vec<f64> = model.start.clone();
    let base = (model.evaluate)(&base_point).ok_or(Ending::NotNumber)?;
    let mut slopes = vec![vec![0.0; n]; base.len()];
    for j in 0..n {
        let step = 1.0;
        let mut point = base_point.clone();
        point[j] += step;
        let moved = (model.evaluate)(&point).ok_or(Ending::NotNumber)?;
        for (row, slope) in slopes.iter_mut().enumerate() {
            slope[j] = (moved[row] - base[row]) / step;
        }
    }
    let constant: Vec<f64> = (0..base.len())
        .map(|row| base[row] - dot(&slopes[row], &base_point))
        .collect();
    // A second point, off every axis, has to agree.
    let check: Vec<f64> = base_point
        .iter()
        .enumerate()
        .map(|(j, v)| v + 1.5 + 0.37 * j as f64)
        .collect();
    let seen = (model.evaluate)(&check).ok_or(Ending::NotNumber)?;
    for row in 0..base.len() {
        let expected = constant[row] + dot(&slopes[row], &check);
        let scale = 1.0 + expected.abs().max(seen[row].abs());
        if (expected - seen[row]).abs() > 1e-6 * scale {
            return Err(Ending::NotLinear);
        }
    }
    let sign = match model.goal {
        Goal::Max => -1.0,
        _ => 1.0,
    };
    let mut rows: Vec<(Vec<f64>, Relation, f64)> = model
        .relations
        .iter()
        .enumerate()
        .map(|(i, relation)| (slopes[i + 1].clone(), *relation, -constant[i + 1]))
        .collect();
    let cost = match model.goal {
        Goal::Value(target) => {
            // Value Of: the objective held at the target as a constraint.
            rows.push((slopes[0].clone(), Relation::Equal, target - constant[0]));
            vec![0.0; n]
        }
        _ => slopes[0].iter().map(|c| sign * c).collect(),
    };
    Ok(Linear {
        cost,
        rows,
        bounds: model.bounds.clone(),
    })
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Solves `linear` by the two-phase simplex method with Bland's rule; the
/// variables shifted to their lower bounds (or split when free).
fn simplex(linear: &Linear, eps: f64) -> Result<Vec<f64>, Ending> {
    let n = linear.cost.len();
    // Each variable as `low + y` (y >= 0), or `y⁺ - y⁻` when it has no
    // lower bound.
    let mut columns: Vec<(usize, f64)> = Vec::new(); // (variable, sign)
    let mut offset = vec![0.0; n];
    for j in 0..n {
        let (low, _) = linear.bounds[j];
        if low.is_finite() {
            offset[j] = low;
            columns.push((j, 1.0));
        } else {
            columns.push((j, 1.0));
            columns.push((j, -1.0));
        }
    }
    let mut rows: Vec<(Vec<f64>, Relation, f64)> = Vec::new();
    for (a, relation, b) in &linear.rows {
        let shifted = b - dot(a, &offset);
        rows.push((columns.iter().map(|(j, s)| a[*j] * s).collect(), *relation, shifted));
    }
    for j in 0..n {
        let (low, high) = linear.bounds[j];
        if high.is_finite() {
            let a: Vec<f64> = columns
                .iter()
                .map(|(k, s)| if *k == j { *s } else { 0.0 })
                .collect();
            let room = if low.is_finite() { high - low } else { high };
            rows.push((a, Relation::AtMost, room));
        }
    }
    let cost: Vec<f64> = columns.iter().map(|(j, s)| linear.cost[*j] * s).collect();
    let y = tableau(&cost, &rows, eps)?;
    let mut x = offset;
    for (k, (j, s)) in columns.iter().enumerate() {
        x[*j] += s * y[k];
    }
    Ok(x)
}

/// Minimises `cost·y` subject to `rows`, `y >= 0`.
fn tableau(cost: &[f64], rows: &[(Vec<f64>, Relation, f64)], eps: f64) -> Result<Vec<f64>, Ending> {
    let n = cost.len();
    let m = rows.len();
    // Columns: n structural, a slack or surplus per inequality, an
    // artificial per row needing one, then the right side.
    let slacks = rows.iter().filter(|(_, r, _)| *r != Relation::Equal).count();
    let width = n + slacks + m + 1;
    let mut t = vec![vec![0.0; width]; m];
    let mut basis = vec![0usize; m];
    let mut artificial = vec![false; width];
    let mut slack_at = n;
    for (i, (a, relation, b)) in rows.iter().enumerate() {
        let flip = if *b < 0.0 { -1.0 } else { 1.0 };
        for j in 0..n {
            t[i][j] = a[j] * flip;
        }
        t[i][width - 1] = b * flip;
        let mut basic = None;
        match relation {
            Relation::AtMost | Relation::AtLeast => {
                let sign = if *relation == Relation::AtMost { 1.0 } else { -1.0 };
                t[i][slack_at] = sign * flip;
                if sign * flip > 0.0 {
                    basic = Some(slack_at);
                }
                slack_at += 1;
            }
            Relation::Equal => {}
        }
        let column = basic.unwrap_or_else(|| {
            let column = n + slacks + i;
            t[i][column] = 1.0;
            artificial[column] = true;
            column
        });
        basis[i] = column;
    }
    // Phase one: minimise the artificials.
    let phase_one: Vec<f64> = (0..width - 1)
        .map(|j| if artificial[j] { 1.0 } else { 0.0 })
        .collect();
    pivot_until(&mut t, &mut basis, &phase_one, &vec![false; width], eps)?;
    let infeasibility: f64 = basis
        .iter()
        .enumerate()
        .filter(|(_, b)| artificial[**b])
        .map(|(i, _)| t[i][width - 1])
        .sum();
    if infeasibility > eps.max(1e-7) * 10.0 {
        return Err(Ending::Infeasible);
    }
    // Artificials still basic at zero leave the basis where they can.
    for i in 0..m {
        if artificial[basis[i]] {
            if let Some(j) = (0..width - 1).find(|&j| !artificial[j] && t[i][j].abs() > eps) {
                pivot(&mut t, &mut basis, i, j);
            }
        }
    }
    let mut phase_two = vec![0.0; width - 1];
    phase_two[..n].copy_from_slice(cost);
    pivot_until(&mut t, &mut basis, &phase_two, &artificial, eps)?;
    let mut y = vec![0.0; n];
    for (i, b) in basis.iter().enumerate() {
        if *b < n {
            y[*b] = t[i][width - 1];
        }
    }
    Ok(y)
}

fn pivot(t: &mut [Vec<f64>], basis: &mut [usize], row: usize, column: usize) {
    let value = t[row][column];
    for v in t[row].iter_mut() {
        *v /= value;
    }
    let pivot_row = t[row].clone();
    for (i, line) in t.iter_mut().enumerate() {
        if i != row {
            let factor = line[column];
            if factor != 0.0 {
                for (v, p) in line.iter_mut().zip(&pivot_row) {
                    *v -= factor * p;
                }
            }
        }
    }
    basis[row] = column;
}

fn pivot_until(
    t: &mut [Vec<f64>],
    basis: &mut [usize],
    cost: &[f64],
    barred: &[bool],
    eps: f64,
) -> Result<(), Ending> {
    let width = cost.len() + 1;
    for _ in 0..50_000 {
        // Reduced costs, from the basis' costs.
        let entering = (0..width - 1).find(|&j| {
            if barred[j] || basis.contains(&j) {
                return false;
            }
            let reduced = cost[j]
                - basis
                    .iter()
                    .enumerate()
                    .map(|(i, b)| cost[*b] * t[i][j])
                    .sum::<f64>();
            reduced < -eps
        });
        let Some(column) = entering else {
            return Ok(());
        };
        let leaving = (0..t.len())
            .filter(|&i| t[i][column] > eps)
            .min_by(|&a, &b| {
                let ra = t[a][width - 1] / t[a][column];
                let rb = t[b][width - 1] / t[b][column];
                ra.partial_cmp(&rb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(basis[a].cmp(&basis[b]))
            });
        let Some(row) = leaving else {
            return Err(Ending::Unbounded);
        };
        pivot(t, basis, row, column);
    }
    Err(Ending::Limit)
}

/// Simplex LP: the linear model solved, whole numbers found by branch and
/// bound, `dif` groups as an assignment of 1..n.
pub fn simplex_lp(model: &mut Model, options: &Options) -> Solution {
    let linear = match linearise(model) {
        Ok(linear) => linear,
        Err(ending) => return stopped(model, ending),
    };
    let eps = 1e-9;
    let mut best: Option<(f64, Vec<f64>)> = None;
    let mut stack = vec![linear.clone()];
    let mut nodes = 0u32;
    let started = std::time::Instant::now();
    let mut ending = Ending::Optimal;
    while let Some(problem) = stack.pop() {
        nodes += 1;
        if nodes > options.iterations.max(1) * 10 || started.elapsed().as_secs_f64() > options.seconds {
            ending = Ending::Limit;
            break;
        }
        let x = match simplex(&problem, eps) {
            Ok(x) => x,
            Err(Ending::Unbounded) if !model.integer.iter().any(|&i| i) => {
                return stopped(model, Ending::Unbounded)
            }
            Err(_) => continue,
        };
        let value = dot(&problem.cost, &x);
        if let Some((found, _)) = &best {
            let gap = options.integer_tolerance * found.abs().max(1e-9);
            if value >= found - gap.max(1e-9) {
                continue;
            }
        }
        let fractional = (0..x.len())
            .filter(|&j| model.integer[j])
            .map(|j| (j, x[j] - x[j].floor()))
            .filter(|(_, part)| *part > 1e-6 && *part < 1.0 - 1e-6)
            .max_by(|a, b| {
                let da = (a.1 - 0.5).abs();
                let db = (b.1 - 0.5).abs();
                db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
            });
        // A `dif` group with two values the same branches on them.
        let clash = model.different.iter().find_map(|group| {
            for (a, &i) in group.iter().enumerate() {
                for &k in &group[a + 1..] {
                    if (x[i].round() - x[k].round()).abs() < 0.5 {
                        return Some((i, k, x[i].round()));
                    }
                }
            }
            None
        });
        match (fractional, clash) {
            (Some((j, _)), _) => {
                let mut down = problem.clone();
                down.bounds[j].1 = down.bounds[j].1.min(x[j].floor());
                let mut up = problem;
                up.bounds[j].0 = up.bounds[j].0.max(x[j].ceil());
                stack.push(down);
                stack.push(up);
            }
            (None, Some((i, k, v))) => {
                // Neither, or just one of them, takes `v`.
                for (pin, other) in [(i, k), (k, i)] {
                    let mut branch = problem.clone();
                    branch.bounds[pin] = (v, v);
                    let mut below = branch.clone();
                    below.bounds[other].1 = below.bounds[other].1.min(v - 1.0);
                    let mut above = branch;
                    above.bounds[other].0 = above.bounds[other].0.max(v + 1.0);
                    stack.push(below);
                    stack.push(above);
                }
                let mut below = problem.clone();
                below.bounds[i].1 = below.bounds[i].1.min(v - 1.0);
                let mut above = problem;
                above.bounds[i].0 = above.bounds[i].0.max(v + 1.0);
                stack.push(below);
                stack.push(above);
            }
            (None, None) => {
                let x: Vec<f64> = x
                    .iter()
                    .enumerate()
                    .map(|(j, v)| if model.integer[j] { v.round() } else { *v })
                    .collect();
                best = Some((value, x));
            }
        }
    }
    match best {
        Some((_, x)) => finish(model, x, ending, nodes, options),
        None if ending == Ending::Limit => stopped(model, Ending::Limit),
        None => stopped(model, Ending::Infeasible),
    }
}

fn stopped(model: &mut Model, ending: Ending) -> Solution {
    let objective = (model.evaluate)(&model.start)
        .map(|values| values[0])
        .unwrap_or(f64::NAN);
    Solution {
        ending,
        values: model.start.clone(),
        objective,
        iterations: 0,
    }
}

/// The solution checked on the sheet itself: a constraint off by more than
/// the precision makes it infeasible.
fn finish(model: &mut Model, x: Vec<f64>, ending: Ending, iterations: u32, options: &Options) -> Solution {
    let Some(values) = (model.evaluate)(&x) else {
        return stopped(model, Ending::NotNumber);
    };
    let scale = values[1..]
        .iter()
        .fold(1.0f64, |most, v| most.max(v.abs()));
    let holds = violation(&model.relations, &values) <= options.precision.max(1e-9) * scale.max(1.0) * 10.0
        && within(&model.bounds, &x, options.precision.max(1e-9) * 10.0);
    Solution {
        ending: if holds { ending } else { Ending::Infeasible },
        objective: values[0],
        values: x,
        iterations,
    }
}

// ------------------------------------------------------------ GRG Nonlinear

/// GRG Nonlinear: constraints folded into an augmented Lagrangian, each
/// subproblem minimised by BFGS over numeric gradients with the bounds
/// projected; whole numbers by rounding then a local search.
pub fn grg_nonlinear(model: &mut Model, options: &Options) -> Solution {
    let n = model.start.len();
    let m = model.relations.len();
    let started = std::time::Instant::now();
    let mut x: Vec<f64> = project(&model.bounds, &model.start);
    let mut multipliers = vec![0.0; m];
    let mut penalty = 10.0;
    let mut iterations = 0u32;
    let mut last: Option<f64> = None;
    let mut ending = Ending::Converged;
    let goal = model.goal;
    let relations = model.relations.clone();
    let bounds = model.bounds.clone();
    for _outer in 0..30 {
        let lagrangian = |values: &[f64], multipliers: &[f64], penalty: f64| -> f64 {
            let mut total = aimed(goal, values[0]);
            for (i, relation) in relations.iter().enumerate() {
                let g = values[i + 1];
                let lambda = multipliers[i];
                total += match relation {
                    Relation::Equal => lambda * g + 0.5 * penalty * g * g,
                    // g <= 0
                    Relation::AtMost => {
                        let s = (lambda + penalty * g).max(0.0);
                        (s * s - lambda * lambda) / (2.0 * penalty)
                    }
                    // g >= 0, as -g <= 0
                    Relation::AtLeast => {
                        let s = (lambda - penalty * g).max(0.0);
                        (s * s - lambda * lambda) / (2.0 * penalty)
                    }
                };
            }
            total
        };
        let mut f = |point: &[f64]| -> Option<f64> {
            (model.evaluate)(point).map(|values| lagrangian(&values, &multipliers, penalty))
        };
        let (found, used) = bfgs(&mut f, &x, &bounds, options, &started);
        iterations += used;
        let Some(found) = found else {
            return stopped(model, Ending::NotNumber);
        };
        x = found;
        let Some(values) = (model.evaluate)(&x) else {
            return stopped(model, Ending::NotNumber);
        };
        for (i, relation) in relations.iter().enumerate() {
            let g = values[i + 1];
            multipliers[i] = match relation {
                Relation::Equal => multipliers[i] + penalty * g,
                Relation::AtMost => (multipliers[i] + penalty * g).max(0.0),
                Relation::AtLeast => (multipliers[i] - penalty * g).max(0.0),
            };
        }
        let off = violation(&relations, &values);
        let objective = aimed(goal, values[0]);
        let settled = last.is_some_and(|before: f64| {
            (objective - before).abs() <= options.convergence * (1.0 + before.abs())
        });
        if off <= options.precision && settled {
            break;
        }
        if off > options.precision {
            penalty = (penalty * 10.0).min(1e9);
        }
        last = Some(objective);
        if iterations >= options.iterations || started.elapsed().as_secs_f64() > options.seconds {
            ending = Ending::Limit;
            break;
        }
    }
    if model.integer.iter().any(|&i| i) {
        x = whole_numbers(model, x, options);
    }
    if let Goal::Value(_) = goal {
        // Value Of is found when the objective reaches its target.
        ending = Ending::Optimal;
    } else if ending == Ending::Converged && n > 0 {
        ending = Ending::Optimal;
    }
    finish(model, x, ending, iterations, options)
}

fn project(bounds: &[(f64, f64)], x: &[f64]) -> Vec<f64> {
    x.iter()
        .zip(bounds)
        .map(|(v, (low, high))| v.max(*low).min(*high))
        .collect()
}

/// Minimises `f` from `start` within `bounds`: projected BFGS with a
/// backtracking line search and central-difference gradients.
fn bfgs(
    f: &mut dyn FnMut(&[f64]) -> Option<f64>,
    start: &[f64],
    bounds: &[(f64, f64)],
    options: &Options,
    started: &std::time::Instant,
) -> (Option<Vec<f64>>, u32) {
    let n = start.len();
    let mut x = project(bounds, start);
    let Some(mut fx) = f(&x) else {
        return (None, 0);
    };
    let gradient = |f: &mut dyn FnMut(&[f64]) -> Option<f64>, x: &[f64]| -> Option<Vec<f64>> {
        let mut g = vec![0.0; n];
        for j in 0..n {
            let h = 1e-6 * (1.0 + x[j].abs());
            let mut up = x.to_vec();
            let mut down = x.to_vec();
            up[j] += h;
            down[j] -= h;
            // At a bound, a one-sided difference.
            let (low, high) = bounds[j];
            let (a, b, width) = if up[j] > high {
                (x.to_vec(), down, h)
            } else if down[j] < low {
                (up, x.to_vec(), h)
            } else {
                (up, down, 2.0 * h)
            };
            g[j] = (f(&a)? - f(&b)?) / width;
        }
        Some(g)
    };
    let Some(mut g) = gradient(f, &x) else {
        return (None, 0);
    };
    let mut inverse = identity(n);
    let mut used = 0;
    for _ in 0..200 {
        used += 1;
        if started.elapsed().as_secs_f64() > options.seconds {
            break;
        }
        // Variables held at a bound the gradient pushes past stay there.
        let free: Vec<bool> = (0..n)
            .map(|j| {
                let (low, high) = bounds[j];
                !((x[j] <= low && g[j] > 0.0) || (x[j] >= high && g[j] < 0.0))
            })
            .collect();
        let mut direction: Vec<f64> = (0..n)
            .map(|i| {
                if !free[i] {
                    return 0.0;
                }
                -(0..n)
                    .filter(|&j| free[j])
                    .map(|j| inverse[i][j] * g[j])
                    .sum::<f64>()
            })
            .collect();
        let mut slope = dot(&direction, &g);
        if slope >= 0.0 {
            inverse = identity(n);
            direction = (0..n).map(|i| if free[i] { -g[i] } else { 0.0 }).collect();
            slope = dot(&direction, &g);
        }
        if direction.iter().all(|d| d.abs() < 1e-14) {
            break;
        }
        let mut step = 1.0;
        let mut next = None;
        for _ in 0..60 {
            let trial = project(
                bounds,
                &x.iter().zip(&direction).map(|(v, d)| v + step * d).collect::<Vec<_>>(),
            );
            if let Some(value) = f(&trial) {
                if value <= fx + 1e-4 * step * slope.min(0.0) {
                    next = Some((trial, value));
                    break;
                }
            }
            step *= 0.5;
        }
        let Some((trial, value)) = next else {
            break;
        };
        let Some(g_next) = gradient(f, &trial) else {
            break;
        };
        let s: Vec<f64> = trial.iter().zip(&x).map(|(a, b)| a - b).collect();
        let y: Vec<f64> = g_next.iter().zip(&g).map(|(a, b)| a - b).collect();
        let sy = dot(&s, &y);
        let change = (fx - value).abs();
        x = trial;
        g = g_next;
        let small = change <= 1e-12 * (1.0 + fx.abs());
        fx = value;
        if sy > 1e-12 {
            // H ← (I - ρsyᵀ) H (I - ρysᵀ) + ρssᵀ
            let rho = 1.0 / sy;
            let hy: Vec<f64> = (0..n).map(|i| dot(&inverse[i], &y)).collect();
            let yhy = dot(&y, &hy);
            for i in 0..n {
                for j in 0..n {
                    inverse[i][j] += -rho * (hy[i] * s[j] + s[i] * hy[j])
                        + (rho * rho * yhy + rho) * s[i] * s[j];
                }
            }
        }
        if small || g.iter().all(|v| v.abs() < 1e-10) {
            break;
        }
    }
    (Some(x), used)
}

fn identity(n: usize) -> Vec<Vec<f64>> {
    (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect()
}

/// The whole-number variables rounded, then each nudged a step either way
/// while that does better and keeps the constraints.
fn whole_numbers(model: &mut Model, mut x: Vec<f64>, options: &Options) -> Vec<f64> {
    for j in 0..x.len() {
        if model.integer[j] {
            x[j] = x[j].round();
        }
    }
    let score = |model: &mut Model, x: &[f64]| -> f64 {
        match (model.evaluate)(x) {
            Some(values) => {
                let off = violation(&model.relations, &values);
                if off > options.precision {
                    1e30 + off
                } else {
                    aimed(model.goal, values[0])
                }
            }
            None => f64::INFINITY,
        }
    };
    let mut best = score(model, &x);
    for _ in 0..200 {
        let mut better = false;
        for j in 0..x.len() {
            if !model.integer[j] {
                continue;
            }
            for delta in [-1.0, 1.0] {
                let mut trial = x.clone();
                trial[j] += delta;
                if !within(&model.bounds, &trial, 0.0) {
                    continue;
                }
                let value = score(model, &trial);
                if value < best - 1e-12 {
                    best = value;
                    x = trial;
                    better = true;
                }
            }
        }
        if !better {
            break;
        }
    }
    x
}

// ------------------------------------------------------------- Evolutionary

/// A small, seeded random number source (xorshift).
struct Random(u64);

impl Random {
    fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        let u = self.next().max(1e-300);
        let v = self.next();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// Evolutionary: a population bred by crossover and mutation within the
/// bounds, unfit members penalised by how far they break constraints,
/// stopped when the best stops improving; the best then polished locally.
pub fn evolutionary(model: &mut Model, options: &Options) -> Solution {
    let n = model.start.len();
    let started = std::time::Instant::now();
    let mut random = Random(options.seed.wrapping_mul(2_862_933_555_777_941_757).wrapping_add(0x9E37_79B9_7F4A_7C15) | 1);
    // Bounds the search keeps to: a variable without one searches round
    // its start.
    let span: Vec<(f64, f64)> = model
        .bounds
        .iter()
        .zip(&model.start)
        .map(|((low, high), v)| {
            let reach = 10.0 * (1.0 + v.abs());
            let low = if low.is_finite() { *low } else { v - reach };
            let high = if high.is_finite() { *high } else { v + reach };
            (low, high)
        })
        .collect();
    let integer = model.integer.clone();
    let tidy = |x: &mut Vec<f64>| {
        for j in 0..n {
            x[j] = x[j].max(span[j].0).min(span[j].1);
            if integer[j] {
                x[j] = x[j].round();
            }
        }
    };
    let fitness = |model: &mut Model, x: &[f64]| -> f64 {
        match (model.evaluate)(x) {
            Some(values) => {
                let off = violation(&model.relations, &values);
                let aim = aimed(model.goal, values[0]);
                if off > options.precision {
                    1e12 * (1.0 + off) + aim.abs().min(1e11)
                } else {
                    aim
                }
            }
            None => f64::INFINITY,
        }
    };
    let size = options.population.clamp(10, 400);
    let mut population: Vec<(Vec<f64>, f64)> = Vec::with_capacity(size);
    let mut first = model.start.clone();
    tidy(&mut first);
    let score = fitness(model, &first);
    population.push((first, score));
    while population.len() < size {
        let mut x: Vec<f64> = span
            .iter()
            .map(|(low, high)| low + random.next() * (high - low))
            .collect();
        tidy(&mut x);
        let score = fitness(model, &x);
        population.push((x, score));
    }
    let by_score = |a: &(Vec<f64>, f64), b: &(Vec<f64>, f64)| {
        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
    };
    population.sort_by(by_score);
    let mut stale = 0;
    let mut generations = 0u32;
    let mut ending = Ending::Converged;
    while stale < 60 {
        generations += 1;
        if generations > options.iterations.max(50) || started.elapsed().as_secs_f64() > options.seconds {
            ending = Ending::Limit;
            break;
        }
        let best_before = population[0].1;
        let mut children = Vec::with_capacity(size);
        let rate = 0.1 + 0.4 * (stale as f64 / 60.0);
        for _ in 0..size / 2 {
            let pick = |random: &mut Random| {
                let a = (random.next() * size as f64) as usize % size;
                let b = (random.next() * size as f64) as usize % size;
                a.min(b)
            };
            let mother = &population[pick(&mut random)].0;
            let father = &population[pick(&mut random)].0;
            let mut child: Vec<f64> = (0..n)
                .map(|j| {
                    let w = random.next();
                    w * mother[j] + (1.0 - w) * father[j]
                })
                .collect();
            for j in 0..n {
                if random.next() < rate {
                    child[j] += random.normal() * 0.1 * (span[j].1 - span[j].0).max(1e-9);
                }
            }
            tidy(&mut child);
            let score = fitness(model, &child);
            children.push((child, score));
        }
        population.extend(children);
        population.sort_by(by_score);
        population.truncate(size);
        if population[0].1 < best_before - options.convergence.max(1e-12) * (1.0 + best_before.abs()) {
            stale = 0;
        } else {
            stale += 1;
        }
    }
    let mut best = population.swap_remove(0).0;
    // Polished: each variable moved while that does better.
    let mut current = fitness(model, &best);
    let mut step: Vec<f64> = span.iter().map(|(low, high)| (high - low) * 0.01).collect();
    for _ in 0..200 {
        let mut better = false;
        for j in 0..n {
            for sign in [-1.0, 1.0] {
                let mut trial = best.clone();
                trial[j] += sign * if integer[j] { 1.0 } else { step[j] };
                tidy(&mut trial);
                let value = fitness(model, &trial);
                if value < current {
                    current = value;
                    best = trial;
                    better = true;
                }
            }
        }
        if !better {
            if step.iter().all(|s| *s < 1e-9) {
                break;
            }
            step.iter_mut().for_each(|s| *s *= 0.5);
        }
    }
    finish(model, best, ending, generations, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        method: fn(&mut Model, &Options) -> Solution,
        start: Vec<f64>,
        goal: Goal,
        relations: Vec<Relation>,
        integer: Vec<bool>,
        bounds: Vec<(f64, f64)>,
        mut f: impl FnMut(&[f64]) -> Vec<f64>,
    ) -> Solution {
        let mut evaluate = |x: &[f64]| Some(f(x));
        let mut model = Model {
            start,
            goal,
            relations,
            integer,
            bounds,
            different: Vec::new(),
            evaluate: &mut evaluate,
        };
        method(&mut model, &Options::default())
    }

    /// Maximise 3x + 5y with x <= 4, 2y <= 12, 3x + 2y <= 18: x 2, y 6, 36.
    #[test]
    fn simplex_solves_the_textbook_product_mix() {
        let found = run(
            simplex_lp,
            vec![0.0, 0.0],
            Goal::Max,
            vec![Relation::AtMost; 3],
            vec![false; 2],
            vec![(0.0, f64::INFINITY); 2],
            |x| vec![3.0 * x[0] + 5.0 * x[1], x[0] - 4.0, 2.0 * x[1] - 12.0, 3.0 * x[0] + 2.0 * x[1] - 18.0],
        );
        assert_eq!(found.ending, Ending::Optimal, "{found:?}");
        assert!((found.objective - 36.0).abs() < 1e-6);
        assert!((found.values[0] - 2.0).abs() < 1e-6 && (found.values[1] - 6.0).abs() < 1e-6);
    }

    /// A diet: minimise 0.6a + b with 10a + 4b >= 20, 5a + 5b >= 20,
    /// 2a + 6b >= 12, a, b >= 0: a 3, b 1, 2.8.
    #[test]
    fn simplex_minimises_with_at_least_constraints_and_says_infeasible() {
        let found = run(
            simplex_lp,
            vec![0.0, 0.0],
            Goal::Min,
            vec![Relation::AtLeast; 3],
            vec![false; 2],
            vec![(0.0, f64::INFINITY); 2],
            |x| {
                vec![
                    0.6 * x[0] + x[1],
                    10.0 * x[0] + 4.0 * x[1] - 20.0,
                    5.0 * x[0] + 5.0 * x[1] - 20.0,
                    2.0 * x[0] + 6.0 * x[1] - 12.0,
                ]
            },
        );
        assert!((found.objective - 2.8).abs() < 1e-6, "{found:?}");
        let none = run(
            simplex_lp,
            vec![0.0],
            Goal::Max,
            vec![Relation::AtLeast, Relation::AtMost],
            vec![false],
            vec![(0.0, f64::INFINITY)],
            |x| vec![x[0], x[0] - 5.0, x[0] - 3.0],
        );
        assert_eq!(none.ending, Ending::Infeasible);
        let unbounded = run(
            simplex_lp,
            vec![0.0],
            Goal::Max,
            vec![],
            vec![false],
            vec![(0.0, f64::INFINITY)],
            |x| vec![x[0]],
        );
        assert_eq!(unbounded.ending, Ending::Unbounded);
        let curved = run(
            simplex_lp,
            vec![1.0],
            Goal::Min,
            vec![],
            vec![false],
            vec![(0.0, f64::INFINITY)],
            |x| vec![x[0] * x[0]],
        );
        assert_eq!(curved.ending, Ending::NotLinear);
    }

    /// The knapsack: values 10, 13, 7, 8, weights 5, 7, 4, 3, into 14: the
    /// last three fill it exactly for 28.
    #[test]
    fn simplex_branches_for_whole_numbers() {
        let values = [10.0, 13.0, 7.0, 8.0];
        let weights = [5.0, 7.0, 4.0, 3.0];
        let found = run(
            simplex_lp,
            vec![0.0; 4],
            Goal::Max,
            vec![Relation::AtMost],
            vec![true; 4],
            vec![(0.0, 1.0); 4],
            |x| vec![dot(&values, x), dot(&weights, x) - 14.0],
        );
        assert_eq!(found.ending, Ending::Optimal, "{found:?}");
        assert!((found.objective - 28.0).abs() < 1e-6, "{found:?}");
    }

    /// Rosenbrock's valley from (-1.2, 1): its minimum 0 at (1, 1).
    #[test]
    fn grg_finds_the_bottom_of_a_curved_valley_and_keeps_a_constraint() {
        let found = run(
            grg_nonlinear,
            vec![-1.2, 1.0],
            Goal::Min,
            vec![],
            vec![false; 2],
            vec![(f64::NEG_INFINITY, f64::INFINITY); 2],
            |x| vec![100.0 * (x[1] - x[0] * x[0]).powi(2) + (1.0 - x[0]).powi(2)],
        );
        assert!(found.objective < 1e-6, "{found:?}");
        assert!((found.values[0] - 1.0).abs() < 1e-3);
        // Largest rectangle with perimeter 20: a 5 by 5 square.
        let found = run(
            grg_nonlinear,
            vec![1.0, 1.0],
            Goal::Max,
            vec![Relation::Equal],
            vec![false; 2],
            vec![(0.0, f64::INFINITY); 2],
            |x| vec![x[0] * x[1], 2.0 * x[0] + 2.0 * x[1] - 20.0],
        );
        assert!((found.objective - 25.0).abs() < 1e-3, "{found:?}");
        // Value Of: x² reaching 2.
        let found = run(
            grg_nonlinear,
            vec![1.0],
            Goal::Value(2.0),
            vec![],
            vec![false],
            vec![(0.0, f64::INFINITY)],
            |x| vec![x[0] * x[0]],
        );
        assert!((found.values[0] - 2f64.sqrt()).abs() < 1e-3, "{found:?}");
    }

    /// A bumpy function GRG would stop on a local dip of: x·sin(x) on 0..12,
    /// its lowest about -11.04 near 11.09.
    #[test]
    fn evolutionary_finds_the_lowest_of_many_dips() {
        let found = run(
            evolutionary,
            vec![1.0],
            Goal::Min,
            vec![],
            vec![false],
            vec![(0.0, 12.0)],
            |x| vec![x[0] * x[0].sin()],
        );
        assert!((found.objective + 11.0407).abs() < 1e-3, "{found:?}");
    }
}
