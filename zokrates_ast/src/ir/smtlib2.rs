use num_bigint::BigUint;
use std::collections::BTreeSet;

use super::*;
use zokrates_field::Field;

use super::expression::LinComb;
use super::expression::QuadComb;
use super::visitor::*;

pub trait SMTLib2 {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result;
}

pub struct SMTLib2Display<'a, 'ast, T>(pub &'a Prog<'ast, T>);

impl<'ast, T: Field> fmt::Display for SMTLib2Display<'_, 'ast, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.to_smtlib2(f)
    }
}

struct VariableCollector {
    variables: BTreeSet<Variable>,
}

impl<T: Field> Visitor<T> for VariableCollector {
    fn visit_variable(&mut self, v: &Variable) {
        self.variables.insert(*v);
    }
}

impl<'ast, T: Field> SMTLib2 for Prog<'ast, T> {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut collector = VariableCollector {
            variables: BTreeSet::<Variable>::new(),
        };
        collector.visit_module(self);
        collector.variables.insert(Variable::one());

        writeln!(f, "; Auto generated by ZoKrates")?;
        writeln!(
            f,
            "; Number of circuit variables: {}",
            collector.variables.len()
        )?;
        writeln!(f, "; Number of equalities: {}", self.statements.len())?;

        writeln!(f, "(declare-const |~prime| Int)")?;
        for v in collector.variables.iter() {
            writeln!(f, "(declare-const |{}| Int)", v)?;
        }

        writeln!(f, "(assert (and")?;
        writeln!(f, "(= |~prime| {})", T::max_value().to_biguint() + 1usize)?;
        writeln!(f, "(= |~one| 1)")?;
        for s in &self.statements {
            s.to_smtlib2(f)?;
            writeln!(f)?;
        }
        write!(f, "))")
    }
}

fn format_prefix_op_smtlib2<T: SMTLib2, Ts: SMTLib2>(
    f: &mut fmt::Formatter,
    op: &str,
    a: &T,
    b: &Ts,
) -> fmt::Result {
    write!(f, "({} ", op)?;
    a.to_smtlib2(f)?;
    write!(f, " ")?;
    b.to_smtlib2(f)?;
    write!(f, ")")
}

impl<'ast, T: Field> SMTLib2 for Statement<'ast, T> {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Statement::Block(..) => unreachable!(),
            Statement::Constraint(ref quad, ref lin, _) => {
                write!(f, "(= (mod ")?;
                quad.to_smtlib2(f)?;
                write!(f, " |~prime|) (mod ")?;
                lin.to_smtlib2(f)?;
                write!(f, " |~prime|))")
            }
            Statement::Directive(ref s) => s.to_smtlib2(f),
            Statement::Log(..) => write!(f, ""),
        }
    }
}

impl<'ast, T: Field> SMTLib2 for Directive<'ast, T> {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "")
    }
}

impl<T: Field> SMTLib2 for QuadComb<T> {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_prefix_op_smtlib2(f, "*", &self.left, &self.right)
    }
}

impl<T: Field> SMTLib2 for LinComb<T> {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.is_zero() {
            true => write!(f, "0"),
            false => {
                if self.0.len() > 1 {
                    write!(f, "(+")?;
                    for expr in self.0.iter() {
                        write!(f, " ")?;
                        format_prefix_op_smtlib2(f, "*", &expr.0, &expr.1.to_biguint())?;
                    }
                    write!(f, ")")
                } else {
                    format_prefix_op_smtlib2(f, "*", &self.0[0].0, &self.0[0].1.to_biguint())
                }
            }
        }
    }
}

impl SMTLib2 for Variable {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "|{}|", self)
    }
}

impl SMTLib2 for BigUint {
    fn to_smtlib2(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}
