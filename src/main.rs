use std::collections::HashMap;

extern crate boolean_expression;

use boolean_expression::{Expr, BDD};

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub enum V {
    A,
    B,
    C,
}

fn main() {
    let mut b = BDD::new();
    let expr = Expr::xor_direct(vec![V::A, V::B, V::C]);
    let f = b.from_expr(&expr);
    println!("{}", b.to_dot(f));

    let mut b = BDD::new();
    let expr_ite = Expr::xor_ite(vec![V::A, V::B, V::C]);
    let f_ite = b.from_expr(&expr_ite);
    println!("{}", b.to_dot(f_ite));
}
