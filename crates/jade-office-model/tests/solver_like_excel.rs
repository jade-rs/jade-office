//! Data ▸ Solver as Excel's: an objective cell maximised, minimised or set
//! to a value by changing variable cells under constraints, by Simplex LP,
//! GRG Nonlinear or Evolutionary; the solution kept or the values put back,
//! an Answer Report, and the model kept in hidden `solver_*` names.

#![cfg(feature = "tiny-skia")]

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::sheet::{CellRef, Workbook};
use jade_office_model::solver::methods::{Ending, Goal};
use jade_office_model::solver::{Constraint, Method, Rel, SolverModel};
use jade_office_model::tiny::TinySkiaBackend;
use jade_office_model::what_if::{WhatIfDialog, WhatIfTool};

fn backend() -> TinySkiaBackend {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let directory = core.join("fonts/Noto_Sans_JP/static");
    TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap()
}

fn book() -> Workbook<TinySkiaBackend> {
    let core = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let bytes = std::fs::read(core.join("public/fixtures/grid-operations-workbook.xlsx")).unwrap();
    Workbook::open(&bytes, "UTC", backend()).unwrap()
}

fn at(column: u32, row: u32) -> CellRef {
    CellRef::new(column, row)
}

/// The textbook product mix: H30 doors, H31 windows; profit 3 and 5 a unit
/// in H32; plant hours H33 (doors), H34 (windows) and H35 (both).
fn product_mix(book: &mut Workbook<TinySkiaBackend>) -> SolverModel {
    book.set_cell(at(7, 29), "0").unwrap();
    book.set_cell(at(7, 30), "0").unwrap();
    book.set_cell(at(7, 31), "=3*H30+5*H31").unwrap();
    book.set_cell(at(7, 32), "=H30").unwrap();
    book.set_cell(at(7, 33), "=2*H31").unwrap();
    book.set_cell(at(7, 34), "=3*H30+2*H31").unwrap();
    book.set_cell(at(8, 32), "4").unwrap();
    book.set_cell(at(8, 33), "12").unwrap();
    book.set_cell(at(8, 34), "18").unwrap();
    SolverModel {
        objective: "$H$32".into(),
        goal: Goal::Max,
        variables: "$H$30:$H$31".into(),
        constraints: vec![Constraint {
            left: "$H$33:$H$35".into(),
            relation: Rel::AtMost,
            right: "$I$33:$I$35".into(),
        }],
        method: Method::SimplexLp,
        ..SolverModel::default()
    }
}

#[test]
fn simplex_solves_the_product_mix_and_one_undo_puts_it_back() {
    let mut book = book();
    let model = product_mix(&mut book);
    let solved = book.solve(&model).unwrap();
    assert_eq!(solved.solution.ending, Ending::Optimal, "{solved:?}");
    assert_eq!(book.display_text(at(7, 29)), "2");
    assert_eq!(book.display_text(at(7, 30)), "6");
    assert_eq!(book.display_text(at(7, 31)), "36");
    assert!(book.undo().unwrap());
    assert_eq!(book.display_text(at(7, 31)), "0");
}

#[test]
fn grg_and_evolutionary_solve_a_curved_model() {
    let mut book = book();
    // The largest area a fence of 20 makes: H30 by H31.
    book.set_cell(at(7, 29), "1").unwrap();
    book.set_cell(at(7, 30), "1").unwrap();
    book.set_cell(at(7, 31), "=H30*H31").unwrap();
    book.set_cell(at(7, 32), "=2*H30+2*H31").unwrap();
    let mut model = SolverModel {
        objective: "H32".into(),
        goal: Goal::Max,
        variables: "H30:H31".into(),
        constraints: vec![Constraint {
            left: "H33".into(),
            relation: Rel::Equal,
            right: "20".into(),
        }],
        method: Method::GrgNonlinear,
        ..SolverModel::default()
    };
    let solved = book.solve(&model).unwrap();
    assert!(
        (solved.solution.objective - 25.0).abs() < 1e-3,
        "{solved:?}"
    );
    // The simplex method refuses a model that is not linear.
    model.method = Method::SimplexLp;
    book.set_cell(at(7, 29), "1").unwrap();
    assert_eq!(
        book.solve(&model).unwrap().solution.ending,
        Ending::NotLinear
    );
    // x·sin(x) on 0..12 has its lowest, about -11.04, near 11.09.
    book.set_cell(at(7, 29), "1").unwrap();
    book.set_cell(at(7, 31), "=H30*SIN(H30)").unwrap();
    let model = SolverModel {
        objective: "H32".into(),
        goal: Goal::Min,
        variables: "H30".into(),
        constraints: vec![Constraint {
            left: "H30".into(),
            relation: Rel::AtMost,
            right: "12".into(),
        }],
        method: Method::Evolutionary,
        ..SolverModel::default()
    };
    let solved = book.solve(&model).unwrap();
    assert!(
        (solved.solution.objective + 11.0407).abs() < 0.01,
        "{solved:?}"
    );
}

#[test]
fn whole_numbers_and_binary_choices_are_kept() {
    let mut book = book();
    // A knapsack: take H30:H33 (0 or 1) of values 10, 13, 7, 8 and weights
    // 5, 7, 4, 3 into 14.
    for (row, (value, weight)) in [(10, 5), (13, 7), (7, 4), (8, 3)].iter().enumerate() {
        book.set_cell(at(7, 29 + row as u32), "0").unwrap();
        book.set_cell(at(8, 29 + row as u32), &value.to_string())
            .unwrap();
        book.set_cell(at(9, 29 + row as u32), &weight.to_string())
            .unwrap();
    }
    book.set_cell(at(7, 34), "=SUMPRODUCT(H30:H33,I30:I33)")
        .unwrap();
    book.set_cell(at(7, 35), "=SUMPRODUCT(H30:H33,J30:J33)")
        .unwrap();
    let model = SolverModel {
        objective: "H35".into(),
        goal: Goal::Max,
        variables: "H30:H33".into(),
        constraints: vec![
            Constraint {
                left: "H36".into(),
                relation: Rel::AtMost,
                right: "14".into(),
            },
            Constraint {
                left: "H30:H33".into(),
                relation: Rel::Bin,
                right: "binary".into(),
            },
        ],
        method: Method::SimplexLp,
        ..SolverModel::default()
    };
    let solved = book.solve(&model).unwrap();
    assert_eq!(solved.solution.ending, Ending::Optimal, "{solved:?}");
    assert_eq!(book.display_text(at(7, 34)), "28");
    let taken: Vec<String> = (29..=32).map(|row| book.display_text(at(7, row))).collect();
    assert_eq!(taken, ["0", "1", "1", "1"]);
}

#[test]
fn the_model_is_kept_in_hidden_names_and_read_back_after_saving() {
    let mut book = book();
    let mut model = product_mix(&mut book);
    model.options.precision = 0.0001;
    model.constraints.push(Constraint {
        left: "$H$30".into(),
        relation: Rel::Int,
        right: "integer".into(),
    });
    assert!(book.set_solver_model(&model).unwrap());
    let sheet = book.sheet_index();
    let names: Vec<&str> = book
        .defined_names()
        .iter()
        .filter(|name| name.local_sheet_index == Some(sheet) && name.hidden)
        .map(|name| name.name.as_str())
        .collect();
    for wanted in [
        "solver_opt",
        "solver_adj",
        "solver_typ",
        "solver_num",
        "solver_lhs2",
        "solver_rel2",
        "solver_rhs2",
        "solver_eng",
        "solver_pre",
    ] {
        assert!(names.contains(&wanted), "{wanted} in {names:?}");
    }
    let rhs = book
        .defined_names()
        .iter()
        .find(|name| name.local_sheet_index == Some(sheet) && name.name == "solver_rhs2")
        .unwrap();
    assert_eq!(rhs.formula, "\"integer\"");
    assert_eq!(book.solver_model(), model);
    let bytes = book.save_bytes().unwrap();
    let mut again = Workbook::open(&bytes, "UTC", backend()).unwrap();
    again.select_sheet(sheet).unwrap();
    assert_eq!(again.solver_model(), model);
    // A shorter list takes the names of the constraints it no longer has.
    model.constraints.pop();
    again.set_solver_model(&model).unwrap();
    assert!(!again
        .defined_names()
        .iter()
        .any(|name| name.name == "solver_lhs2" && name.local_sheet_index == Some(sheet)));
}

#[test]
fn the_dialog_solves_restores_and_writes_an_answer_report() {
    let mut book = book();
    let model = product_mix(&mut book);
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::Solver,
        &book,
        jade_office_model::sheet::span(at(7, 31), at(7, 31)),
        at(7, 31),
    );
    dialog.form_mut().set_text("objective", "$H$32");
    dialog.form_mut().set_text("variables", "$H$30:$H$31");
    // Add Constraint: $H$33:$H$35 <= $I$33:$I$35.
    dialog.press("add", &mut book).unwrap();
    dialog.form_mut().set_text("left", "$H$33:$H$35");
    dialog.form_mut().set_text("right", "$I$33:$I$35");
    dialog.press("ok", &mut book).unwrap();
    assert!(dialog.form().alert.is_none(), "{:?}", dialog.form().alert);
    // Simplex LP.
    let method = dialog
        .form()
        .items
        .iter()
        .position(|item| item.id == "method")
        .unwrap();
    dialog.click(method, 1, &mut book).unwrap();
    assert!(dialog.press("solve", &mut book).unwrap().wrote);
    assert_eq!(book.display_text(at(7, 31)), "36");
    // Restore Original Values.
    let keep = dialog
        .form()
        .items
        .iter()
        .position(|item| item.id == "keep")
        .unwrap();
    dialog.click(keep, 1, &mut book).unwrap();
    assert!(dialog.press("ok", &mut book).unwrap().close);
    assert_eq!(book.display_text(at(7, 31)), "0");
    assert_eq!(book.solver_model().constraints, model.constraints);
    // Solved again, kept, with an Answer Report on a sheet of its own.
    let sheets = book.sheet_names().len();
    let mut dialog = WhatIfDialog::open(
        WhatIfTool::Solver,
        &book,
        jade_office_model::sheet::span(at(7, 31), at(7, 31)),
        at(7, 31),
    );
    assert_eq!(dialog.form().text("objective"), "$H$32");
    dialog.press("solve", &mut book).unwrap();
    let answer = dialog
        .form()
        .items
        .iter()
        .position(|item| item.id == "answer")
        .unwrap();
    dialog.click(answer, 0, &mut book).unwrap();
    dialog.press("ok", &mut book).unwrap();
    assert_eq!(book.sheet_names().len(), sheets + 1);
    assert_eq!(book.sheet_name(), "Answer Report 1");
    assert_eq!(book.display_text(at(0, 0)), "Answer Report");
}
