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
pub mod program;
pub mod tape;

use crate::builder::{Builder, Instruction};
use crate::builtins::Unreachable;
use crate::cpu::{Addr, Dispatch, Halt};
use crate::debug_info::Dump;
use crate::id::Id;
use crate::program::Program;
use crate::tape::{AsClearedWriter, UnexpectedEndError};

use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::{self, MaybeUninit};
use core::ops::Deref;

/// A machine for which programs can be built.
///
/// A machine is a CPU, a tape on which to write the program, and some RAM.
/// Obviously all of those are virtual here, as our goal is to make high-level
/// virtual machines.
#[derive(Clone, Copy, Debug)]
pub struct Machine<Cpu, Tape, Ram> {
    cpu: Cpu,
    tape: Tape,
    ram: Ram,
}

impl<Cpu, Tape, Ram> Machine<Cpu, Tape, Ram>
where
    Tape: AsClearedWriter,
{
    /// Returns a new machine, given a CPU, a tape and some RAM.
    ///
    /// This is the entry point to all of NAAM.
    #[inline(always)]
    pub fn new(cpu: Cpu, tape: Tape, ram: Ram) -> Self {
        Self { cpu, tape, ram }
    }

    /// Consumes the machine and returns a new program.
    ///
    /// The program's instructions cannot borrow from the RAM.
    pub fn program<Env, Error>(
        mut self,
        build: impl FnOnce(&mut Builder<'_, Cpu, Ram, Env>, &mut Ram) -> Result<(), Error>,
    ) -> Result<Program<Cpu, Tape, Ram, Env>, Error>
    where
        Cpu: Dispatch<Ram, Env>,
        Error: From<UnexpectedEndError>,
    {
        let mut builder = Builder::new(self.cpu, &mut self.tape);
        build(&mut builder, &mut self.ram)?;
        builder.emit(Unreachable)?;
        unsafe {
            let debug_info = builder.into_debug_info();
            Ok(Program::new(self.cpu, self.tape, debug_info, self.ram))
        }
    }
}

/// How to execute an operation, the main piece of code for end users.
pub trait Execute<'tape, Ram, Env>: 'tape + Copy + Dump<'tape> + Sized
where
    Env: ?Sized,
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
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    ) -> Destination<'tape>;
}

/// The runner, which allows resolving tape offsets during execution.
pub struct Runner<'tape, Ram, Env>
where
    Env: ?Sized,
{
    tape: *const u8,
    #[cfg(debug_assertions)]
    len: usize,
    marker: marker<fn(&mut Ram, &mut Env)>,
    id: Id<'tape>,
}

impl<'tape, Ram, Env> Runner<'tape, Ram, Env>
where
    Env: ?Sized,
{
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
            marker,
            id: Id::default(),
        }
    }
}

impl<'tape, Ram, Env> Clone for Runner<'tape, Ram, Env>
where
    Env: ?Sized,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<'tape, Ram, Env> Copy for Runner<'tape, Ram, Env> where Env: ?Sized {}

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
