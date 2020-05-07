use crate::zir::*;
use std::collections::HashMap;
use zir::folder::*;
use zokrates_field::Field;

#[derive(Default)]
pub struct UintOptimizer<'ast, T: Field> {
    ids: HashMap<ZirAssignee<'ast>, UMetadata<T>>,
}

impl<'ast, T: Field> UintOptimizer<'ast, T> {
    pub fn new() -> Self {
        UintOptimizer {
            ids: HashMap::new(),
        }
    }

    pub fn optimize(p: ZirProgram<'ast, T>) -> ZirProgram<'ast, T> {
        UintOptimizer::new().fold_program(p)
    }

    fn register(&mut self, a: ZirAssignee<'ast>, m: UMetadata<T>) {
        self.ids.insert(a, m);
    }
}

fn force_reduce<'ast, T: Field>(e: UExpression<'ast, T>) -> UExpression<'ast, T> {
    UExpression {
        metadata: Some(UMetadata {
            should_reduce: Some(true),
            ..e.metadata.unwrap()
        }),
        ..e
    }
}

fn force_no_reduce<'ast, T: Field>(e: UExpression<'ast, T>) -> UExpression<'ast, T> {
    UExpression {
        metadata: Some(UMetadata {
            should_reduce: Some(false),
            ..e.metadata.unwrap()
        }),
        ..e
    }
}

impl<'ast, T: Field> Folder<'ast, T> for UintOptimizer<'ast, T> {
    fn fold_uint_expression(&mut self, e: UExpression<'ast, T>) -> UExpression<'ast, T> {
        if e.metadata.is_some() {
            return e;
        }

        let max_bitwidth = T::get_required_bits() - 1;

        let range = e.bitwidth;

        let range_max: T = (2_usize.pow(range as u32) - 1).into();

        assert!(range < max_bitwidth / 2);

        let inner = e.inner;

        use self::UExpressionInner::*;

        let res = match inner {
            Value(v) => Value(v).annotate(range).metadata(UMetadata {
                max: v.into(),
                should_reduce: Some(false),
            }),
            Identifier(id) => Identifier(id.clone()).annotate(range).metadata(
                self.ids
                    .get(&Variable::uint(id.clone(), range))
                    .cloned()
                    .expect(&format!("identifier should have been defined: {}", id)),
            ),
            Add(box left, box right) => {
                // reduce the two terms
                let left = self.fold_uint_expression(left);
                let right = self.fold_uint_expression(right);

                let left_max = left.metadata.clone().unwrap().max;
                let right_max = right.metadata.clone().unwrap().max;

                let (should_reduce_left, should_reduce_right, max) = left_max
                    .checked_add(&right_max)
                    .map(|max| (false, false, max))
                    .unwrap_or_else(|| {
                        range_max
                            .clone()
                            .checked_add(&right_max)
                            .map(|max| (true, false, max))
                            .unwrap_or_else(|| {
                                left_max
                                    .checked_add(&range_max.clone())
                                    .map(|max| (false, true, max))
                                    .unwrap_or_else(|| (true, true, range_max.clone() + range_max))
                            })
                    });

                let left = if should_reduce_left {
                    force_reduce(left)
                } else {
                    left
                };
                let right = if should_reduce_right {
                    force_reduce(right)
                } else {
                    right
                };

                UExpression::add(left, right).metadata(UMetadata {
                    max,

                    should_reduce: Some(false),
                })
            }
            Sub(box left, box right) => {
                // let `target` the target bitwidth of `left` and `right`
                // `0 <= left <= max_left`
                // `0 <= right <= max_right`
                // `- max_right <= left - right <= max_right`
                // let `n_bits_left` the number of bits needed to represent `max_left`
                // let `n = max(n_bits_left, target)`
                // let offset = 2**n`

                // `2**n - max_left <= a - b + 2 ** n <= bound  where  bound = max_left + offset`

                // If ´bound < N´, we set we return `bound` as the max of ´left - right`
                // Else we start again, reducing `left`. In this case `max_left` becomes `2**target - 1`
                // Else we start again, reducing `right`. In this case `offset` becomes `2**target`
                // Else we start again reducing both. In this case `bound` becomes `2**(target+1) - 1` which is always
                // smaller or equal to N for target in {8, 16, 32}

                // reduce the two terms
                let left = self.fold_uint_expression(left);
                let right = self.fold_uint_expression(right);

                let left_max = left.metadata.clone().unwrap().max;
                let right_bitwidth = right.metadata.clone().unwrap().bitwidth();

                let offset =
                    T::from(2u32).pow(std::cmp::max(right_bitwidth, range as u32) as usize);
                let target_offset = T::from(2u32).pow(range);

                let (should_reduce_left, should_reduce_right, max) = left_max
                    .checked_add(&offset)
                    .map(|max| (false, false, max))
                    .unwrap_or_else(|| {
                        range_max
                            .clone()
                            .checked_add(&offset)
                            .map(|max| (true, false, max))
                            .unwrap_or_else(|| {
                                left_max
                                    .checked_add(&target_offset.clone())
                                    .map(|max| (false, true, max))
                                    .unwrap_or_else(|| {
                                        (true, true, range_max.clone() + target_offset)
                                    })
                            })
                    });

                let left = if should_reduce_left {
                    force_reduce(left)
                } else {
                    left
                };
                let right = if should_reduce_right {
                    force_reduce(right)
                } else {
                    right
                };

                UExpression::sub(left, right).metadata(UMetadata {
                    max,
                    should_reduce: Some(false),
                })
            }
            Xor(box left, box right) => {
                // reduce the two terms
                let left = self.fold_uint_expression(left);
                let right = self.fold_uint_expression(right);

                UExpression::xor(force_reduce(left), force_reduce(right)).metadata(UMetadata {
                    max: range_max.clone(),
                    should_reduce: Some(false),
                })
            }
            And(box left, box right) => {
                // reduce the two terms
                let left = self.fold_uint_expression(left);
                let right = self.fold_uint_expression(right);

                UExpression::and(force_reduce(left), force_reduce(right)).metadata(UMetadata {
                    max: range_max.clone(),
                    should_reduce: Some(false),
                })
            }
            Or(box left, box right) => {
                // reduce the two terms
                let left = self.fold_uint_expression(left);
                let right = self.fold_uint_expression(right);

                UExpression::or(force_reduce(left), force_reduce(right)).metadata(UMetadata {
                    max: range_max.clone(),
                    should_reduce: Some(false),
                })
            }
            Mult(box left, box right) => {
                // reduce the two terms
                let left = self.fold_uint_expression(left);
                let right = self.fold_uint_expression(right);

                let left_max = left.metadata.clone().unwrap().max;
                let right_max = right.metadata.clone().unwrap().max;

                let (should_reduce_left, should_reduce_right, max) = left_max
                    .checked_mul(&right_max)
                    .map(|max| (false, false, max))
                    .unwrap_or_else(|| {
                        range_max
                            .clone()
                            .checked_mul(&right_max)
                            .map(|max| (true, false, max))
                            .unwrap_or_else(|| {
                                left_max
                                    .checked_mul(&range_max.clone())
                                    .map(|max| (false, true, max))
                                    .unwrap_or_else(|| (true, true, range_max.clone() * range_max))
                            })
                    });

                let left = if should_reduce_left {
                    force_reduce(left)
                } else {
                    left
                };
                let right = if should_reduce_right {
                    force_reduce(right)
                } else {
                    right
                };

                UExpression::mult(left, right).metadata(UMetadata {
                    max,
                    should_reduce: Some(false),
                })
            }
            Not(box e) => {
                let e = self.fold_uint_expression(e);

                UExpressionInner::Not(box force_reduce(e))
                    .annotate(range)
                    .metadata(UMetadata {
                        max: range_max.clone(),
                        should_reduce: Some(false),
                    })
            }
            LeftShift(box e, box by) => {
                // reduce the two terms
                let e = self.fold_uint_expression(e);
                let by = self.fold_field_expression(by);

                UExpression::left_shift(force_reduce(e), by).metadata(UMetadata {
                    max: range_max.clone(),
                    should_reduce: Some(true),
                })
            }
            RightShift(box e, box by) => {
                // reduce the two terms
                let e = self.fold_uint_expression(e);
                let by = self.fold_field_expression(by);

                UExpression::right_shift(force_reduce(e), by).metadata(UMetadata {
                    max: range_max.clone(),
                    should_reduce: Some(false),
                })
            }
            IfElse(box condition, box consequence, box alternative) => {
                let consequence = self.fold_uint_expression(consequence);
                let alternative = self.fold_uint_expression(alternative);

                let consequence_max = consequence.metadata.clone().unwrap().max;
                let alternative_max = alternative.metadata.clone().unwrap().max;

                let max = std::cmp::max(
                    consequence_max.into_big_uint(),
                    alternative_max.into_big_uint(),
                );

                UExpression::if_else(condition, consequence, alternative).metadata(UMetadata {
                    max: max.into(),
                    should_reduce: Some(false),
                })
            }
        };

        assert!(res.metadata.is_some());

        res
    }

    fn fold_statement(&mut self, s: ZirStatement<'ast, T>) -> Vec<ZirStatement<'ast, T>> {
        match s {
            ZirStatement::Definition(a, e) => {
                let e = self.fold_expression(e);

                let e = match e {
                    ZirExpression::Uint(i) => {
                        let i = force_no_reduce(i);
                        self.register(a.clone(), i.metadata.clone().unwrap());
                        ZirExpression::Uint(i)
                    }
                    e => e,
                };
                vec![ZirStatement::Definition(a, e)]
            }
            // we need to put back in range to return
            ZirStatement::Return(expressions) => vec![ZirStatement::Return(
                expressions
                    .into_iter()
                    .map(|e| match e {
                        ZirExpression::Uint(e) => {
                            let e = self.fold_uint_expression(e);

                            let e = UExpression {
                                metadata: Some(UMetadata {
                                    should_reduce: Some(true),
                                    ..e.metadata.unwrap()
                                }),
                                ..e
                            };

                            ZirExpression::Uint(e)
                        }
                        e => self.fold_expression(e),
                    })
                    .collect(),
            )],
            ZirStatement::MultipleDefinition(lhs, rhs) => match rhs {
                ZirExpressionList::FunctionCall(key, arguments, ty) => match key.clone().id {
                    "_U32_FROM_BITS" => {
                        assert_eq!(lhs.len(), 1);
                        self.register(
                            lhs[0].clone(),
                            UMetadata {
                                max: T::from(2).pow(32) - T::from(1),
                                should_reduce: Some(false),
                            },
                        );
                        vec![ZirStatement::MultipleDefinition(
                            lhs,
                            ZirExpressionList::FunctionCall(key, arguments, ty),
                        )]
                    }
                    _ => vec![ZirStatement::MultipleDefinition(
                        lhs,
                        ZirExpressionList::FunctionCall(
                            key,
                            arguments
                                .into_iter()
                                .map(|e| self.fold_expression(e))
                                .collect(),
                            ty,
                        ),
                    )],
                },
            },
            // we need to put back in range to assert
            ZirStatement::Condition(lhs, rhs) => {
                match (self.fold_expression(lhs), self.fold_expression(rhs)) {
                    (ZirExpression::Uint(lhs), ZirExpression::Uint(rhs)) => {
                        let lhs_metadata = lhs.metadata.clone().unwrap();
                        let rhs_metadata = rhs.metadata.clone().unwrap();
                        vec![ZirStatement::Condition(
                            lhs.metadata(UMetadata {
                                should_reduce: Some(true),
                                ..lhs_metadata
                            })
                            .into(),
                            rhs.metadata(UMetadata {
                                should_reduce: Some(true),
                                ..rhs_metadata
                            })
                            .into(),
                        )]
                    }
                    (lhs, rhs) => vec![ZirStatement::Condition(lhs, rhs)],
                }
            }
            s => fold_statement(self, s),
        }
    }

    fn fold_parameter(&mut self, p: Parameter<'ast>) -> Parameter<'ast> {
        let id = match p.id.get_type() {
            Type::Uint(bitwidth) => {
                self.register(
                    p.id.clone(),
                    UMetadata {
                        max: T::from(2_u32).pow(bitwidth) - T::from(1),
                        should_reduce: Some(false),
                    },
                );
                p.id
            }
            _ => p.id,
        };

        Parameter {
            id: self.fold_variable(id),
            ..p
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zokrates_field::Bn128Field;
    use zokrates_field::Pow;

    // #[should_panic]
    // #[test]
    // fn existing_metadata() {
    //     let e = UExpressionInner::Identifier("foo".into())
    //         .annotate(32)
    //         .metadata(UMetadata::with_max(2_u32.pow(33_u32) - 1));

    //     let mut optimizer: UintOptimizer<Bn128Field> = UintOptimizer::new();

    //     let _ = optimizer.fold_uint_expression(e.clone());
    // }

    #[test]
    fn add() {
        // max(left + right) = max(left) + max(right)

        let left: UExpression<Bn128Field> = UExpressionInner::Identifier("foo".into())
            .annotate(32)
            .metadata(UMetadata::with_max(42u32));

        let right = UExpressionInner::Identifier("foo".into())
            .annotate(32)
            .metadata(UMetadata::with_max(33u32));

        assert_eq!(
            UintOptimizer::new()
                .fold_uint_expression(UExpression::add(left, right))
                .metadata
                .unwrap()
                .max,
            75u32.into()
        );
    }

    #[test]
    fn sub() {
        // `left` and `right` are smaller than the target
        let left: UExpression<Bn128Field> = UExpressionInner::Identifier("a".into())
            .annotate(32)
            .metadata(UMetadata::with_max(42u32));

        let right = UExpressionInner::Identifier("b".into())
            .annotate(32)
            .metadata(UMetadata::with_max(33u32));

        assert_eq!(
            UintOptimizer::new()
                .fold_uint_expression(UExpression::sub(left, right))
                .metadata
                .unwrap()
                .max,
            Bn128Field::from(2u32).pow(32) + Bn128Field::from(42)
        );

        // `left` and `right` are larger than the target but no readjustment is required
        let left: UExpression<Bn128Field> = UExpressionInner::Identifier("a".into())
            .annotate(32)
            .metadata(UMetadata::with_max(u64::MAX as u128));

        let right = UExpressionInner::Identifier("b".into())
            .annotate(32)
            .metadata(UMetadata::with_max(u64::MAX as u128));

        assert_eq!(
            UintOptimizer::new()
                .fold_uint_expression(UExpression::sub(left, right))
                .metadata
                .unwrap()
                .max,
            Bn128Field::from(2).pow(64) + Bn128Field::from(u64::MAX as u128)
        );

        // `left` and `right` are larger than the target and needs to be readjusted
        let left: UExpression<Bn128Field> = UExpressionInner::Identifier("a".into())
            .annotate(32)
            .metadata(UMetadata::with_max(
                Bn128Field::from(2u32).pow(Bn128Field::get_required_bits() - 1)
                    - Bn128Field::from(1),
            ));

        let right = UExpressionInner::Identifier("b".into())
            .annotate(32)
            .metadata(UMetadata::with_max(42u32));

        assert_eq!(
            UintOptimizer::new()
                .fold_uint_expression(UExpression::sub(left, right))
                .metadata
                .unwrap()
                .max,
            Bn128Field::from(2u32).pow(32) * Bn128Field::from(2) - Bn128Field::from(1)
        );
    }

    #[test]
    fn if_else() {
        // `left` and `right` are smaller than the target
        let consequence: UExpression<Bn128Field> = UExpressionInner::Identifier("a".into())
            .annotate(32)
            .metadata(UMetadata::with_max(42u32));

        let alternative = UExpressionInner::Identifier("b".into())
            .annotate(32)
            .metadata(UMetadata::with_max(33u32));

        assert_eq!(
            UintOptimizer::new()
                .fold_uint_expression(UExpression::if_else(
                    BooleanExpression::Value(true),
                    consequence,
                    alternative
                ))
                .metadata
                .unwrap()
                .max,
            Bn128Field::from(42)
        );
    }
}
