use std::future::Future;

use crate::{
    result::{Error, FlowControlError, FlowControlVariant},
    step_tool::Step,
};

#[doc(hidden)]
pub enum BranchOutcome<T> {
    Value(T),
    Yielded,
    Skipped,
}

#[doc(hidden)]
pub async fn __run_parallel_branch<T, F>(fut: F) -> Result<BranchOutcome<T>, Error>
where
    F: Future<Output = Result<T, Error>>,
{
    match fut.await {
        Ok(value) => Ok(BranchOutcome::Value(value)),
        Err(Error::Interrupt(mut flow)) => {
            let outcome = match flow.variant {
                FlowControlVariant::StepGenerator => BranchOutcome::Yielded,
                FlowControlVariant::ParallelSkip => BranchOutcome::Skipped,
            };
            flow.acknowledge();
            Ok(outcome)
        }
        Err(err) => Err(err),
    }
}

#[doc(hidden)]
pub async fn __finish_parallel<T>(
    step: &Step,
    targeted_request: bool,
    branch_skipped: bool,
    branch_yielded: bool,
    values: Option<T>,
) -> Result<T, Error> {
    if branch_yielded {
        return Err(Error::Interrupt(FlowControlError::step_generator()));
    }

    if targeted_request && branch_skipped {
        step.__mark_missing_target();
        return Err(Error::Interrupt(FlowControlError::step_generator()));
    }

    values.ok_or_else(|| Error::Interrupt(FlowControlError::step_generator()))
}

/// Runs a small number of step-producing branches in logical parallel while
/// preserving Rust-native tuple outputs.
///
/// The first argument is the step helper that owns execution state, followed by
/// two or more branch expressions. Each branch can use `?` with step APIs and
/// yields one element of the returned tuple.
pub use crate::__inngest_group_parallel as parallel;

#[doc(hidden)]
pub fn __targeted_request(step: &Step) -> bool {
    step.__is_targeted_request()
}

#[doc(hidden)]
pub fn __enter_parallel_scope(step: &Step) -> crate::step_tool::ParallelScopeGuard {
    step.__enter_parallel_scope()
}

#[macro_export]
macro_rules! __inngest_group_parallel {
    ($step:expr => $a:expr, $b:expr $(,)?) => {{
        async {
            let __step = &$step;
            let __targeted_request = $crate::group::__targeted_request(__step);
            let __parallel_scope = $crate::group::__enter_parallel_scope(__step);
            let mut __branch_skipped = false;
            let mut __branch_yielded = false;
            let mut __value_a = None;
            let mut __value_b = None;

            match $crate::group::__run_parallel_branch(async {
                ::std::result::Result::<_, $crate::result::Error>::Ok($a)
            })
            .await?
            {
                $crate::group::BranchOutcome::Value(value) => __value_a = Some(value),
                $crate::group::BranchOutcome::Yielded => __branch_yielded = true,
                $crate::group::BranchOutcome::Skipped => __branch_skipped = true,
            }

            if !(__targeted_request && __branch_yielded) {
                match $crate::group::__run_parallel_branch(async {
                    ::std::result::Result::<_, $crate::result::Error>::Ok($b)
                })
                .await?
                {
                    $crate::group::BranchOutcome::Value(value) => __value_b = Some(value),
                    $crate::group::BranchOutcome::Yielded => __branch_yielded = true,
                    $crate::group::BranchOutcome::Skipped => __branch_skipped = true,
                }
            }

            drop(__parallel_scope);

            $crate::group::__finish_parallel(
                __step,
                __targeted_request,
                __branch_skipped,
                __branch_yielded,
                match (__value_a, __value_b) {
                    (Some(value_a), Some(value_b)) => Some((value_a, value_b)),
                    _ => None,
                },
            )
            .await
        }
    }};
    ($step:expr => $a:expr, $b:expr, $c:expr $(,)?) => {{
        async {
            let __step = &$step;
            let __targeted_request = $crate::group::__targeted_request(__step);
            let __parallel_scope = $crate::group::__enter_parallel_scope(__step);
            let mut __branch_skipped = false;
            let mut __branch_yielded = false;
            let mut __value_a = None;
            let mut __value_b = None;
            let mut __value_c = None;

            match $crate::group::__run_parallel_branch(async {
                ::std::result::Result::<_, $crate::result::Error>::Ok($a)
            })
            .await?
            {
                $crate::group::BranchOutcome::Value(value) => __value_a = Some(value),
                $crate::group::BranchOutcome::Yielded => __branch_yielded = true,
                $crate::group::BranchOutcome::Skipped => __branch_skipped = true,
            }

            if !(__targeted_request && __branch_yielded) {
                match $crate::group::__run_parallel_branch(async {
                    ::std::result::Result::<_, $crate::result::Error>::Ok($b)
                })
                .await?
                {
                    $crate::group::BranchOutcome::Value(value) => __value_b = Some(value),
                    $crate::group::BranchOutcome::Yielded => __branch_yielded = true,
                    $crate::group::BranchOutcome::Skipped => __branch_skipped = true,
                }
            }

            if !(__targeted_request && __branch_yielded) {
                match $crate::group::__run_parallel_branch(async {
                    ::std::result::Result::<_, $crate::result::Error>::Ok($c)
                })
                .await?
                {
                    $crate::group::BranchOutcome::Value(value) => __value_c = Some(value),
                    $crate::group::BranchOutcome::Yielded => __branch_yielded = true,
                    $crate::group::BranchOutcome::Skipped => __branch_skipped = true,
                }
            }

            drop(__parallel_scope);

            $crate::group::__finish_parallel(
                __step,
                __targeted_request,
                __branch_skipped,
                __branch_yielded,
                match (__value_a, __value_b, __value_c) {
                    (Some(value_a), Some(value_b), Some(value_c)) => {
                        Some((value_a, value_b, value_c))
                    }
                    _ => None,
                },
            )
            .await
        }
    }};
}
