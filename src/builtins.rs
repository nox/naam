use crate::{Destination, Execute, Pc, Runner};

#[derive(Clone, Copy, Debug)]
pub struct Nop;

impl<'tape, Ram, Env> Execute<'tape, Ram, Env> for Nop
where
    Env: ?Sized,
{
    fn execute(
        pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Ram, Env>,
        _ram: &mut Ram,
        _env: &mut Env,
    ) -> Destination<'tape> {
        Ok(pc.next())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Unreachable;

impl<'tape, Ram, Env> Execute<'tape, Ram, Env> for Unreachable
where
    Env: ?Sized,
{
    fn execute(
        _pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Ram, Env>,
        _ram: &mut Ram,
        _env: &mut Env,
    ) -> Destination<'tape> {
        panic!("reached unreachable tape")
    }
}

mod should_be_derived {
    use super::*;

    use crate::debug_info::{Dump, Dumper};

    use core::fmt;

    impl<'tape> Dump<'tape> for Nop {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: &Dumper<'tape>) -> fmt::Result {
            fmt::Debug::fmt(self, fmt)
        }
    }

    impl<'tape> Dump<'tape> for Unreachable {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: &Dumper<'tape>) -> fmt::Result {
            fmt::Debug::fmt(self, fmt)
        }
    }
}
