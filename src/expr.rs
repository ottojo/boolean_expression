// boolean_expression: a Rust crate for Boolean expressions and BDDs.
//
// Copyright (c) 2016 Chris Fallin <cfallin@c1f.net>. Released under the MIT
// License.
//

use std::cmp::Ord;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use simplify;

/// An `Expr` is a simple Boolean logic expression. It may contain terminals
/// (i.e., free variables), constants, and the following fundamental operations:
/// AND, OR, NOT.
///
/// ```
/// use std::collections::HashMap;
/// use boolean_expression::Expr;
///
/// #[derive(Clone, Hash, Eq, PartialEq, Debug)]
/// enum RiverCrossing { Chicken, Fox, Grain }
///
/// let chicken = Expr::Terminal(RiverCrossing::Chicken);
/// let fox_and_grain = Expr::Terminal(RiverCrossing::Fox) & Expr::Terminal(RiverCrossing::Grain);
///
/// let allowed = (!chicken.clone() & fox_and_grain.clone()) | (chicken & !fox_and_grain);
/// let items = [
///    (RiverCrossing::Grain, true),
///    (RiverCrossing::Fox, true),
/// ].iter().cloned().collect();
///
/// // nobody gets eaten!
/// assert!(allowed.evaluate(&items));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    /// A terminal (free variable). This expression node represents a value that
    /// is not known until evaluation time.
    Terminal(T),
    /// A boolean constant: true or false.
    Const(bool),

    /// The logical complement of the contained expression argument.
    Not(Box<Expr<T>>),

    /// The logical AND of the two expression arguments.
    And(Box<Expr<T>>, Box<Expr<T>>),

    /// The logical OR of the two expression arguments.
    Or(Box<Expr<T>>, Box<Expr<T>>),

    /// The Logical XOR (exactly one) of three variables.
    Xor(T, T, T),
}

impl<T> Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    /// Returns `true` if this `Expr` is a terminal.
    pub fn is_terminal(&self) -> bool {
        match self {
            &Expr::Terminal(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if this `Expr` is a constant.
    pub fn is_const(&self) -> bool {
        match self {
            &Expr::Const(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if this `Expr` is a NOT node.
    pub fn is_not(&self) -> bool {
        match self {
            &Expr::Not(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if this `Expr` is an AND node.
    pub fn is_and(&self) -> bool {
        match self {
            &Expr::And(_, _) => true,
            _ => false,
        }
    }

    /// Returns `true` if this `Expr` is an OR node.
    pub fn is_or(&self) -> bool {
        match self {
            &Expr::Or(_, _) => true,
            _ => false,
        }
    }

    /// Builds a NOT node around an argument, consuming the argument
    /// expression.
    pub fn not(e: Expr<T>) -> Expr<T> {
        Expr::Not(Box::new(e))
    }

    /// Builds an AND node around two arguments, consuming the argument
    /// expressions.
    pub fn and(e1: Expr<T>, e2: Expr<T>) -> Expr<T> {
        Expr::And(Box::new(e1), Box::new(e2))
    }

    /// Builds an OR node around two arguments, consuming the argument
    /// expressions.
    pub fn or(e1: Expr<T>, e2: Expr<T>) -> Expr<T> {
        Expr::Or(Box::new(e1), Box::new(e2))
    }

    pub fn xor(e1: Expr<T>, e2: Expr<T>) -> Expr<T> {
        let nand = !(e1.clone() & e2.clone());
        let or = e1 | e2;
        nand & or
    }

    pub fn xor_direct(e1: T, e2: T, e3: T) -> Expr<T> {
        Expr::Xor(e1, e2, e3)
    }

    /// Evaluates the expression with a particular set of terminal assignments.
    /// If any terminals are not assigned, they default to `false`.
    pub fn evaluate(&self, vals: &HashMap<T, bool>) -> bool {
        match self {
            &Expr::Terminal(ref t) => *vals.get(t).unwrap_or(&false),
            &Expr::Const(val) => val,
            &Expr::And(ref a, ref b) => a.evaluate(vals) && b.evaluate(vals),
            &Expr::Or(ref a, ref b) => a.evaluate(vals) || b.evaluate(vals),
            &Expr::Not(ref x) => !x.evaluate(vals),
            &Expr::Xor(ref a, ref b, ref c) => {
                let a = *vals.get(a).unwrap_or(&false);
                let b = *vals.get(b).unwrap_or(&false);
                let c = *vals.get(c).unwrap_or(&false);
                (a || b || c) && (!a || !b) && (!a || !c) && (!b || !c)
            }
        }
    }

    /// Evaluates the expression using the provided function to map terminals
    /// (variables) to boolean values. This is a generalization of
    /// [`Expr::evaluate`], where the variable lookup in a hashmap is replaced
    /// with an arbitrary computation.
    ///
    ///```
    /// use boolean_expression::Expr;
    ///
    /// let expression = Expr::Terminal(10) | Expr::Terminal(3);
    ///
    /// // check if the expression satisfies a predicate
    /// assert!(expression.evaluate_with(|&x| x > 5));
    /// ```
    pub fn evaluate_with<F>(&self, f: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        self.evaluate_with1(&f)
    }

    fn evaluate_with1<F>(&self, f: &F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        match self {
            Expr::Terminal(t) => f(t),
            Expr::Const(val) => *val,
            Expr::And(a, b) => a.evaluate_with1(f) && b.evaluate_with1(f),
            Expr::Or(a, b) => a.evaluate_with1(f) || b.evaluate_with1(f),
            Expr::Not(x) => !x.evaluate_with1(f),
            Expr::Xor(a, b, c) => {
                let a = f(a);
                let b = f(b);
                let c = f(c);
                (a || b || c) && (!a || !b) && (!a || !c) && (!b || !c)
            }
        }
    }

    /// Simplify an expression in a relatively cheap way using well-known logic
    /// identities.
    ///
    /// This function performs certain reductions using DeMorgan's Law,
    /// distribution of ANDs over ORs, and certain identities involving
    /// constants, to simplify an expression. The result will always be in
    /// sum-of-products form: nodes will always appear in order (from
    /// expression tree root to leaves) `OR -> AND -> NOT`. In other words,
    /// `AND` nodes may only have `NOT` nodes (or terminals or constants) as
    /// children, and `NOT` nodes may only have terminals or constants as
    /// children.
    ///
    /// This function explicitly does *not* perform any sort of minterm reduction.
    /// The terms may thus be redundant (i.e., `And(a, b)` may appear twice), and
    /// combinable terms may exist (i.e., `And(a, b)` and `And(a, Not(b))` may
    /// appear in the `OR`'d list of terms, where these could be combined to
    /// simply `a` but are not). For example:
    ///
    /// ```
    /// use boolean_expression::Expr;
    ///
    /// // This simplifies using DeMorgan's Law:
    /// let expr = Expr::not(Expr::or(Expr::Terminal(0), Expr::Terminal(1)));
    /// let simplified = expr.simplify_via_laws();
    /// assert_eq!(simplified,
    ///     Expr::and(Expr::not(Expr::Terminal(0)),
    ///               Expr::not(Expr::Terminal(1))));
    ///
    /// // This doesn't simplify, though:
    /// let expr = Expr::or(
    ///             Expr::and(Expr::Terminal(0), Expr::Terminal(1)),
    ///             Expr::and(Expr::Terminal(0), Expr::not(Expr::Terminal(1))));
    /// let simplified = expr.clone().simplify_via_laws();
    /// assert_eq!(simplified, expr);
    /// ```
    ///
    /// For better (but more expensive) simplification, see `simplify_via_bdd()`.
    pub fn simplify_via_laws(self) -> Expr<T> {
        simplify::simplify_via_laws(self)
    }

    /// Simplify an expression via a roundtrip through a `BDD`. This procedure
    /// is more effective than `Expr::simplify_via_laws()`, but more expensive.
    /// This roundtrip will implicitly simplify an arbitrarily complicated
    /// function (by construction, as the BDD is built), and then find a
    /// quasi-minimal set of terms using cubelist-based reduction. For example:
    ///
    /// ```
    /// use boolean_expression::Expr;
    ///
    /// // `simplify_via_laws()` cannot combine these terms, but
    /// // `simplify_via_bdd()` will:
    /// let expr = Expr::or(
    ///             Expr::and(Expr::Terminal(0), Expr::Terminal(1)),
    ///             Expr::and(Expr::Terminal(0), Expr::not(Expr::Terminal(1))));
    /// let simplified = expr.simplify_via_bdd();
    /// assert_eq!(simplified, Expr::Terminal(0));
    /// ```
    pub fn simplify_via_bdd(self) -> Expr<T> {
        simplify::simplify_via_bdd(self)
    }

    /// Map terminal values using the specified mapping function.
    pub fn map<F, R>(&self, f: F) -> Expr<R>
    where
        F: Fn(&T) -> R,
        R: Clone + Debug + Eq + Hash,
    {
        self.map1(&f)
    }

    fn map1<F, R>(&self, f: &F) -> Expr<R>
    where
        F: Fn(&T) -> R,
        R: Clone + Debug + Eq + Hash,
    {
        match self {
            &Expr::Terminal(ref t) => Expr::Terminal(f(t)),
            &Expr::Const(val) => Expr::Const(val),
            &Expr::Not(ref n) => Expr::not(n.map1(f)),
            &Expr::And(ref a, ref b) => Expr::and(a.map1(f), b.map1(f)),
            &Expr::Or(ref a, ref b) => Expr::or(a.map1(f), b.map1(f)),
            &Expr::Xor(ref a, ref b, ref c) => Expr::xor_direct(f(a), f(b), f(c)),
        }
    }
}

impl<T> Not for Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::not(self)
    }
}

impl<T> BitAnd<Expr<T>> for Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    type Output = Self;

    fn bitand(self, rhs: Expr<T>) -> Self::Output {
        Self::and(self, rhs)
    }
}

impl<T> BitAndAssign<Expr<T>> for Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    fn bitand_assign(&mut self, rhs: Expr<T>) {
        *self = Self::and(self.clone(), rhs);
    }
}

impl<T> BitOr<Expr<T>> for Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    type Output = Self;

    fn bitor(self, rhs: Expr<T>) -> Self::Output {
        Self::or(self, rhs)
    }
}

impl<T> BitOrAssign<Expr<T>> for Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    fn bitor_assign(&mut self, rhs: Expr<T>) {
        *self = Self::or(self.clone(), rhs);
    }
}

impl<T> BitXor<Expr<T>> for Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    type Output = Self;

    fn bitxor(self, rhs: Expr<T>) -> Self::Output {
        Self::xor(self, rhs)
    }
}

impl<T> BitXorAssign<Expr<T>> for Expr<T>
where
    T: Clone + Debug + Eq + Hash,
{
    fn bitxor_assign(&mut self, rhs: Expr<T>) {
        *self = Self::xor(self.clone(), rhs);
    }
}
