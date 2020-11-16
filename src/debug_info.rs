//! Infrastructure to dump programs for debugging purposes.

use crate::id::Id;
use crate::Offset;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::fmt::{self, Debug};
use core::mem::{self, MaybeUninit};

#[cfg(feature = "macros")]
pub use naam_macros::Dump;

/// Trait for values that can be dumped.
///
/// This is like the `Debug` trait, except it takes a dumper to resolve
/// offsets when dumping a program.
pub trait Dump<'tape> {
    /// Dumps a value.
    ///
    /// Use the given dumper to resolve offsets stored in values.
    fn dump(&self, fmt: &mut fmt::Formatter, dumper: Dumper<'tape>) -> fmt::Result;
}

/// A dumper.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Dumper<'tape> {
    tape: *const MaybeUninit<usize>,
    #[allow(dead_code)]
    id: Id<'tape>,
}

impl<'tape> Dumper<'tape> {
    /// Takes a dumpable value and return a bridge that can be passed
    /// to methods expecting values that implement `Debug`.
    pub fn debug<'a, T: Dump<'tape>>(self, value: &'a T) -> DumpDebugBridge<'a, 'tape, T> {
        DumpDebugBridge(value, self)
    }
}

/// A bridge to use dumpable values in `Debug`.
pub struct DumpDebugBridge<'a, 'tape, T>(&'a T, Dumper<'tape>);

impl<'tape, T> Debug for DumpDebugBridge<'_, 'tape, T>
where
    T: Dump<'tape>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.dump(fmt, self.1)
    }
}

impl<'tape> Dump<'tape> for Offset<'tape> {
    fn dump(&self, fmt: &mut fmt::Formatter, dumper: Dumper<'tape>) -> fmt::Result {
        write!(
            fmt,
            "{:?} /* {:p} */",
            self,
            (dumper.base() as *const u8).wrapping_add(self.value),
        )
    }
}

impl Dumper<'_> {
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
    #[cfg(feature = "alloc")]
    instructions: Vec<DebugInstruction>,
}

impl DebugInfo {
    #[cfg(feature = "alloc")]
    pub(crate) unsafe fn push<'tape, I>(&mut self, offset: usize)
    where
        I: Dump<'tape>,
    {
        unsafe fn dump<'tape, I>(
            ptr: *const MaybeUninit<usize>,
            fmt: &mut fmt::Formatter,
            dumper: Dumper<'tape>,
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

impl<'tape> Dump<'tape> for DebugInfo {
    fn dump(
        &self,
        fmt: &mut fmt::Formatter,
        #[cfg_attr(not(feature = "alloc"), allow(unused_variables))] dumper: Dumper<'tape>,
    ) -> fmt::Result {
        impl<'tape> Dump<'tape> for DebugInstruction {
            fn dump(&self, fmt: &mut fmt::Formatter, dumper: Dumper<'tape>) -> fmt::Result {
                // This is fine as long as DebugInfo and this type stay private and we
                // they don't outlive the program they come from.
                unsafe {
                    let dump = mem::transmute::<
                        _,
                        unsafe fn(_, &mut fmt::Formatter, Dumper<'tape>) -> fmt::Result,
                    >(self.1);
                    dump(dumper.tape.add(self.0), fmt, dumper)?;
                }
                Ok(())
            }
        }

        let mut tuple = fmt.debug_tuple("Tape");
        #[cfg(feature = "alloc")]
        for instruction in &self.instructions {
            tuple.field(&format_args!("{:?}", &dumper.debug(instruction)));
        }
        #[cfg(not(feature = "alloc"))]
        tuple.field(&(..));
        tuple.finish()
    }
}

struct DebugInstruction(usize, *const ());
