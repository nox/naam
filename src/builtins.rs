use crate::debug_info::{Dump, Dumper};
use crate::{Destination, Execute, Pc, Runner};

use core::fmt;

#[derive(Clone, Copy, Debug)]
pub struct Unreachable;

impl<'tape> Dump<'tape> for Unreachable {
    fn dump(&self, fmt: &mut fmt::Formatter, _dumper: &Dumper<'tape>) -> fmt::Result {
        fmt::Debug::fmt(self, fmt)
    }
}

impl<'tape, Env, In> Execute<'tape, Env, In> for Unreachable
where
    In: ?Sized,
{
    fn execute(
        _pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Env, In>,
        _env: &mut Env,
        _input: &mut In,
    ) -> Destination<'tape> {
        panic!("reached unreachable tape")
    }
}
