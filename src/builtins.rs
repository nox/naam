//! Built-in operations.

use crate::{Destination, Execute, Pc, Runner};

/// The classic “nop” operation, which does nothing and just continues with
/// the next operation on tape.
#[derive(Clone, Copy, Debug)]
pub struct Nop;

impl<'tape, Ram> Execute<'tape, Ram> for Nop
where
    Ram: ?Sized,
{
    #[inline(always)]
    fn execute(pc: Pc<'tape, Self>, _runner: Runner<'tape>, _ram: &mut Ram) -> Destination<'tape> {
        Ok(pc.next())
    }
}

/// The unreachable operation, which always panic.
#[derive(Clone, Copy, Debug)]
pub struct Unreachable;

impl<'tape, Ram> Execute<'tape, Ram> for Unreachable
where
    Ram: ?Sized,
{
    #[inline(always)]
    fn execute(_pc: Pc<'tape, Self>, _runner: Runner<'tape>, _ram: &mut Ram) -> Destination<'tape> {
        panic!("reached unreachable tape")
    }
}

mod should_be_derived {
    use super::*;

    use crate::debug_info::{Dump, Dumper};

    use core::fmt;

    impl<'tape> Dump<'tape> for Nop {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: Dumper<'tape>) -> fmt::Result {
            fmt::Debug::fmt(self, fmt)
        }
    }

    impl<'tape> Dump<'tape> for Unreachable {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: Dumper<'tape>) -> fmt::Result {
            fmt::Debug::fmt(self, fmt)
        }
    }
}
