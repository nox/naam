//! Building programs.

use crate::cpu::{Dispatch, DispatchToken, GetDispatchToken};
use crate::debug_info::{DebugInfo, Dump, Dumper};
use crate::id::Id;
use crate::tape::{AsClearedWriter, UnexpectedEndError, Writer};
use crate::{Execute, Offset};
use core::fmt;
use core::marker::PhantomData as marker;
use core::mem;
use core::ptr;

/// A program builder. Passed to the closure given to `Machine::program`.
pub struct Builder<'tape, Cpu, Ram, Env>
where
    Env: ?Sized,
{
    cpu: Cpu,
    writer: &'tape mut dyn Writer,
    debug_info: DebugInfo,
    #[allow(dead_code)]
    id: Id<'tape>,
    marker: marker<fn(&mut Ram, &mut Env)>,
}

impl<'tape, Cpu, Ram, Env> Builder<'tape, Cpu, Ram, Env>
where
    Cpu: Dispatch<Ram, Env>,
    Env: ?Sized,
{
    /// Emits an operation, which must be supported by the builder's CPU.
    ///
    /// # Panics
    ///
    /// This method panics if `Op`'s alignment exceeds `usize`'s.
    pub fn emit<Op>(&mut self, op: Op) -> Result<(), UnexpectedEndError>
    where
        Cpu: GetDispatchToken<'tape, Op, Ram, Env>,
        Op: Execute<'tape, Ram, Env>,
        Env: 'tape,
    {
        let instruction = Instruction {
            token: <Cpu as GetDispatchToken<Op, Ram, Env>>::get_dispatch_token(self.cpu),
            op,
        };

        if mem::align_of::<Instruction<Op>>() != mem::size_of::<usize>() {
            panic!("instruction is over-aligned");
        }

        let size_in_words = mem::size_of_val(&instruction) / mem::size_of::<usize>();
        #[cfg(feature = "std")]
        let offset = self.writer.word_offset();
        unsafe {
            let slice = self.writer.take(size_in_words)?;
            ptr::write(slice.as_mut_ptr() as *mut _, instruction);
            #[cfg(feature = "std")]
            self.debug_info.push::<Instruction<Op>>(offset);
        }
        Ok(())
    }

    /// Returns the current offset in the tape.
    ///
    /// The current offset is the distance between the beginning of the tape
    /// and the end of the operation that was last written.
    #[inline(always)]
    pub fn offset(&self) -> Offset<'tape> {
        Offset {
            value: self
                .writer
                .word_offset()
                .wrapping_mul(mem::size_of::<usize>()),
            id: Id::default(),
        }
    }

    #[inline(always)]
    pub(crate) fn new<Tape>(cpu: Cpu, tape: &'tape mut Tape) -> Self
    where
        Tape: AsClearedWriter,
    {
        Self {
            writer: tape.as_cleared_writer(),
            cpu,
            debug_info: DebugInfo::default(),
            id: Id::default(),
            marker,
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn into_debug_info(self) -> DebugInfo {
        self.debug_info
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct Instruction<Op> {
    pub(crate) token: DispatchToken,
    pub(crate) op: Op,
}

impl<'tape, Op> Dump<'tape> for Instruction<Op>
where
    Op: Dump<'tape>,
{
    fn dump(&self, fmt: &mut fmt::Formatter, dumper: Dumper<'tape>) -> fmt::Result {
        fmt::Pointer::fmt(&self, fmt)?;
        fmt.write_str(": ")?;
        self.op.dump(fmt, dumper)
    }
}
