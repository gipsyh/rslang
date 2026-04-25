use rslang::{
    AssertionKind, BinaryOp, Edge, EventControl, Expr, PortDirection, ProcedureKind, SignalKind,
    Stmt,
};

#[test]
fn lowers_mul_design_into_rust_ir() {
    let design = rslang::parse_file("./tests/fvbench/multiplier/multiplier.sv")
        .expect("parse mul.sv with slang");
    let module = design.module("multiplier").expect("multiplier module");

    assert_eq!(module.parameters.len(), 2);
    assert_eq!(module.parameters[0].name, "WL");
    assert_eq!(module.parameters[0].value.as_deref(), Some("16"));
    assert_eq!(module.parameters[1].name, "WS");
    assert_eq!(module.parameters[1].value.as_deref(), Some("8"));

    assert_eq!(module.ports.len(), 6);
    assert_eq!(module.port("clk").unwrap().direction, PortDirection::In);
    assert_eq!(module.port("ix").unwrap().ty, "logic[7:0]");
    assert_eq!(module.nets.len(), 6);

    let variable_names: Vec<_> = module
        .variables
        .iter()
        .map(|signal| signal.name.as_str())
        .collect();
    assert_eq!(
        variable_names,
        ["x", "y", "ma", "mb", "na", "nb", "nc", "nd"]
    );
    assert!(
        module
            .variables
            .iter()
            .all(|signal| signal.kind == SignalKind::Variable)
    );

    assert_eq!(module.procedures.len(), 1);
    let procedure = &module.procedures[0];
    assert_eq!(procedure.kind, ProcedureKind::Always);

    let Stmt::Timed { event, stmt, .. } = &procedure.body else {
        panic!("always body should be timed");
    };
    let EventControl::Signal { edge, expr, .. } = event else {
        panic!("always event should be signal event");
    };
    assert_eq!(*edge, Edge::Pos);
    assert!(matches!(
        expr,
        Expr::NamedValue { symbol, .. } if symbol.name == "clk"
    ));

    let Stmt::Block { statements, .. } = stmt.as_ref() else {
        panic!("timed body should lower to a block");
    };
    assert_eq!(statements.len(), 1);
    assert!(matches!(statements[0], Stmt::If { .. }));

    let mut assignments = 0;
    let mut saw_multiply = false;
    let mut saw_assertion = false;
    procedure.body.for_each(&mut |stmt| match stmt {
        Stmt::Assign { right, .. } => {
            assignments += 1;
            if contains_binary_op(right, BinaryOp::Multiply) {
                saw_multiply = true;
            }
        }
        Stmt::Assert {
            kind, condition, ..
        } => {
            saw_assertion =
                *kind == AssertionKind::Assert && contains_binary_op(condition, BinaryOp::Equality);
        }
        _ => {}
    });

    assert_eq!(assignments, 16);
    assert!(saw_multiply, "expected multiplication assignments");
    assert!(saw_assertion, "expected assert(ma == mb)");
}

fn contains_binary_op(expr: &Expr, needle: BinaryOp) -> bool {
    match expr {
        Expr::Binary {
            op, left, right, ..
        } => {
            *op == needle
                || contains_binary_op(left, needle.clone())
                || contains_binary_op(right, needle)
        }
        Expr::Unary { expr, .. } | Expr::Conversion { expr, .. } => {
            contains_binary_op(expr, needle)
        }
        Expr::Assignment { left, right, .. } => {
            contains_binary_op(left, needle.clone()) || contains_binary_op(right, needle)
        }
        Expr::NamedValue { .. } | Expr::IntegerLiteral { .. } | Expr::Unknown { .. } => false,
    }
}
