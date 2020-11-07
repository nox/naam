use crate::id::Id;
use crate::{Instruction, Offset};

use core::fmt::{self, Debug};
use core::mem::{self, MaybeUninit};

pub trait Dump<'tape> {
    fn dump(&self, fmt: &mut fmt::Formatter, dumper: &Dumper<'tape>) -> fmt::Result;
}

pub struct Dumper<'tape> {
    tape: *const MaybeUninit<usize>,
    #[allow(dead_code)]
    id: Id<'tape>,
}

impl<'tape> Dumper<'tape> {
    pub fn debug<'a, T: Dump<'tape>>(&'a self, value: &'a T) -> DumpDebugBridge<'a, 'tape, T> {
        DumpDebugBridge(value, self)
    }
}

pub struct DumpDebugBridge<'a, 'tape, T>(&'a T, &'a Dumper<'tape>);

impl<'tape, T: Dump<'tape>> Debug for DumpDebugBridge<'_, 'tape, T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.dump(fmt, self.1)
    }
}

impl<'tape> Dump<'tape> for Offset<'tape> {
    fn dump(&self, fmt: &mut fmt::Formatter, dumper: &Dumper<'tape>) -> fmt::Result {
        write!(
            fmt,
            "[base + {}] /* {:p} */",
            self.value,
            dumper.base().wrapping_add(self.value),
        )
    }
}

impl<'tape> Dumper<'tape> {
    pub(crate) unsafe fn new(code: &[MaybeUninit<usize>]) -> Self {
        Self {
            tape: code.as_ptr(),
            id: Id::default(),
        }
    }

    pub(crate) fn base(&self) -> *const MaybeUninit<usize> {
        self.tape
    }
}

#[derive(Default)]
pub(crate) struct DebugInfo {
    #[cfg(feature = "std")]
    instructions: Vec<DebugInstruction>,
}

impl DebugInfo {
    #[cfg(feature = "std")]
    pub(crate) unsafe fn push<'tape, I>(&mut self, offset: usize)
    where
        I: Dump<'tape>,
    {
        unsafe fn dump<'tape, I>(
            ptr: *const MaybeUninit<usize>,
            fmt: &mut fmt::Formatter,
            dumper: &Dumper<'tape>,
        ) -> fmt::Result
        where
            I: Dump<'tape>,
        {
            (&*(ptr as *const I)).dump(fmt, dumper)
        }

        self.instructions
            .push(DebugInstruction(offset, dump::<I> as *const ()));
    }
}

impl<'tape, Op> Dump<'tape> for Instruction<Op>
where
    Op: Dump<'tape>,
{
    fn dump(&self, fmt: &mut fmt::Formatter, dumper: &Dumper<'tape>) -> fmt::Result {
        fmt::Pointer::fmt(&self, fmt)?;
        fmt.write_str(": ")?;
        self.operands.dump(fmt, dumper)
    }
}

impl<'tape> Dump<'tape> for DebugInfo {
    fn dump(
        &self,
        fmt: &mut fmt::Formatter,
        #[cfg_attr(not(feature = "std"), allow(unused_variables))] dumper: &Dumper<'tape>,
    ) -> fmt::Result {
        impl<'tape> Dump<'tape> for DebugInstruction {
            fn dump(&self, fmt: &mut fmt::Formatter, dumper: &Dumper<'tape>) -> fmt::Result {
                // This is fine as long as DebugInfo and this type stay private and we
                // they don't outlive the program they come from.
                unsafe {
                    let dump = mem::transmute::<
                        _,
                        unsafe fn(_, &mut fmt::Formatter, &Dumper<'tape>) -> fmt::Result,
                    >(self.1);
                    dump(dumper.tape.add(self.0), fmt, dumper)?;
                }
                Ok(())
            }
        }

        let mut tuple = fmt.debug_tuple("Tape");
        #[cfg(feature = "std")]
        for instruction in &self.instructions {
            tuple.field(&format_args!("{:?}", &dumper.debug(instruction)));
        }
        #[cfg(not(feature = "std"))]
        tuple.field(&(..));
        tuple.finish()
    }
}

struct DebugInstruction(usize, *const ());
