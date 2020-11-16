//! Built-in operations.

use crate::debug_info::Dump;
use crate::{Destination, Execute, Pc, Runner};

// Hack so that #[derive(Dump)] works in naam itself.
use crate as naam;

/// The classic “nop” operation, which does nothing and just continues with
/// the next operation on tape.
#[derive(Clone, Copy, Debug, Dump)]
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
#[derive(Clone, Copy, Debug, Dump)]
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
