//! **N**ox's **A**bstract **A**bstract **M**achine
//!
//! Highly experimental framework to design higher-level virtual machines
//! fearlessly.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod builder;
pub mod builtins;
pub mod cpu;
pub mod debug_info;
mod id;
pub mod tape;

use crate::builder::{Build, Builder, Instruction};
use crate::builtins::Unreachable;
use crate::cpu::{Addr, Dispatch, Halt};
use crate::debug_info::{DebugInfo, Dump, Dumper};
use crate::id::Id;
use crate::tape::AsClearedWriter;

use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::{self, MaybeUninit};
use core::ops::Deref;
use stable_deref_trait::StableDeref;

/// A compiled program.
pub struct Program<Cpu, Tape, Code> {
    cpu: Cpu,
    tape: Tape,
    debug_info: DebugInfo,
    code: Code,
    not_sync: marker<*mut ()>,
}

impl<Cpu, Tape, Code> Program<Cpu, Tape, Code>
where
    Cpu: for<'ram> Dispatch<<<Code as Deref>::Target as Build<Cpu>>::Ram>,
    Tape: AsClearedWriter,
    Code: StableDeref,
    <Code as Deref>::Target: Build<Cpu>,
{
    /// Returns a new program built from the given code.
    pub fn new(
        cpu: Cpu,
        mut tape: Tape,
        code: Code,
    ) -> Result<Program<Cpu, Tape, Code>, <<Code as Deref>::Target as Build<Cpu>>::Error> {
        let mut builder = Builder::new(cpu, &mut tape);
        code.build(&mut builder)?;
        builder.emit(Unreachable)?;
        unsafe {
            let debug_info = builder.into_debug_info();
            Ok(Self {
                cpu,
                tape,
                debug_info,
                code,
                not_sync: marker,
            })
        }
    }

    /// Runs the program with some RAM.
    pub fn run(&self, ram: &mut <<Code as Deref>::Target as Build<Cpu>>::Ram) {
        let tape = self.tape.as_ref();
        unsafe {
            let runner = Runner::new(tape);
            let addr = Addr {
                token: &*(tape.as_ptr() as *const _),
                id: runner.id,
            };
            self.cpu.dispatch(addr, runner, ram)
        }
    }

    /// Gets a reference to the code used by the program.
    #[inline(always)]
    pub fn code(&self) -> &Code {
        &self.code
    }
}

impl<Cpu, Tape, Code> fmt::Debug for Program<Cpu, Tape, Code>
where
    Cpu: Debug,
    Code: Debug,
    Tape: AsRef<[MaybeUninit<usize>]>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let dumper = unsafe { Dumper::new(self.tape.as_ref()) };
        fmt.debug_struct("Machine")
            .field("cpu", &self.cpu)
            .field("code", &self.code)
            .field("tape", &dumper.debug(&self.debug_info))
            .finish()
    }
}

/// How to execute an operation, the main piece of code for end users.
pub trait Execute<'tape, Ram>: 'tape + Copy + Dump<'tape> + Sized
where
    Ram: ?Sized,
{
    /// Executes the operation.
    ///
    /// Operations are free to mutate both the RAM and the environment provided
    /// when the program was run.
    ///
    /// Use the runner to resolve tape offsets stored in the operation, and the
    /// program counter to continue execution to the next operation.
    ///
    /// **Note:** As the CPU is the entity responsible for dispatching
    /// operations and most CPUs wrap calls to that function in a separate
    /// unsafe function, users should probably mark this method as inline.
    fn execute(pc: Pc<'tape, Self>, runner: Runner<'tape>, ram: &mut Ram) -> Destination<'tape>;
}

/// The runner, which allows resolving tape offsets during execution.
#[derive(Clone, Copy)]
pub struct Runner<'tape> {
    tape: *const u8,
    #[cfg(debug_assertions)]
    len: usize,
    id: Id<'tape>,
}

impl<'tape> Runner<'tape> {
    /// Resolves a tape offset to a physical address.
    #[inline(always)]
    pub fn resolve_offset(self, offset: Offset<'tape>) -> Addr<'tape> {
        debug_assert!(offset.value < self.len);
        debug_assert!(offset.value % mem::align_of::<usize>() == 0);
        unsafe {
            let byte = self.tape.add(offset.value);
            Addr {
                token: &*(byte as *const _),
                id: offset.id,
            }
        }
    }

    /// Returns the error token to return from the program altogether.
    #[inline(always)]
    pub fn halt(self) -> Halt<'tape> {
        Halt { id: self.id }
    }

    #[inline(always)]
    fn new(tape: &'tape [MaybeUninit<usize>]) -> Self {
        Self {
            tape: tape.as_ptr() as *const u8,
            #[cfg(debug_assertions)]
            len: tape.len().wrapping_mul(mem::size_of::<usize>()),
            id: Id::default(),
        }
    }
}

/// The program counter.
///
/// This represents the current position of the CPU in the program and
/// lets the user access the contents of the operation currently executed.
#[repr(transparent)]
pub struct Pc<'tape, Op> {
    instruction: &'tape Instruction<Op>,
    id: Id<'tape>,
}

impl<'tape, Op> Pc<'tape, Op> {
    /// Returns the physical address of the operation currently executed.
    #[inline(always)]
    pub fn current(self) -> Addr<'tape> {
        Addr {
            token: &self.instruction.token,
            id: self.id,
        }
    }

    /// Returns the physical address of the next operation in the program.
    #[inline(always)]
    pub fn next(self) -> Addr<'tape> {
        unsafe {
            let end = (self.instruction as *const Instruction<Op>).add(1);
            Addr {
                token: &*(end as *const _),
                id: self.id,
            }
        }
    }

    /// Creates a new program counter out of a physical address.
    ///
    /// This is only useful for CPU (remember, virtual ones) designers.
    #[inline(always)]
    pub unsafe fn from_addr(addr: Addr<'tape>) -> Self {
        Self {
            instruction: &*(addr.token as *const _ as *const _),
            id: addr.id,
        }
    }
}

impl<'tape, Op> Clone for Pc<'tape, Op> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<'tape, Op> Copy for Pc<'tape, Op> {}

impl<'tape, Op> Deref for Pc<'tape, Op> {
    type Target = Op;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.instruction.op
    }
}

/// A destination for the next step the CPU should take.
///
/// This type alias only exists so that simple programs need only one import
/// instead of two.
pub type Destination<'tape> = Result<Addr<'tape>, Halt<'tape>>;

/// A tape offset.
///
/// Offsets are always guaranteed to refer to the start of an operation
/// in the program. They can be converted to `usize` through the `From` trait.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Offset<'tape> {
    value: usize,
    id: Id<'tape>,
}

impl<'tape> From<Offset<'tape>> for usize {
    #[inline(always)]
    fn from(offset: Offset<'tape>) -> usize {
        offset.value
    }
}

impl<'tape> Debug for Offset<'tape> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "[base + {}]", self.value)
    }
}
