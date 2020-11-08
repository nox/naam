//! **N**ox's **A**bstract **A**bstract **M**achine

#![cfg_attr(not(feature = "std"), no_std)]

pub mod builder;
pub mod builtins;
pub mod cpu;
pub mod debug_info;
mod id;
pub mod program;
pub mod tape;

use crate::builder::{Builder, Instruction};
use crate::cpu::{Addr, Dispatch, Halt};
use crate::debug_info::Dump;
use crate::id::Id;
use crate::program::Program;
use crate::tape::{AsClearedWriter, UnexpectedEndError};

use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::{self, MaybeUninit};
use core::ops::Deref;

#[derive(Clone, Copy, Debug)]
pub struct Machine<Cpu, Tape, Ram> {
    cpu: Cpu,
    tape: Tape,
    ram: Ram,
}

impl<Cpu, Tape, Ram> Machine<Cpu, Tape, Ram> {
    #[inline(always)]
    pub fn new(cpu: Cpu, tape: Tape, ram: Ram) -> Self {
        Self { cpu, tape, ram }
    }

    pub fn program<Env, Error>(
        mut self,
        build: impl FnOnce(&mut Builder<'_, Cpu, Ram, Env>, &mut Ram) -> Result<(), Error>,
    ) -> Result<Program<Cpu, Tape, Ram, Env>, Error>
    where
        Cpu: Dispatch<Ram, Env>,
        Tape: AsClearedWriter,
        Error: From<UnexpectedEndError>,
    {
        let mut builder = Builder::new(self.cpu, &mut self.tape);
        build(&mut builder, &mut self.ram)?;
        builder.emit(self.cpu.unreachable())?;
        unsafe {
            let debug_info = builder.into_debug_info();
            Ok(Program::new(self.cpu, self.tape, debug_info, self.ram))
        }
    }
}

pub trait Execute<'tape, Ram, Env>: 'tape + Copy + Dump<'tape> + Sized
where
    Env: ?Sized,
{
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    ) -> Destination<'tape>;
}

#[repr(transparent)]
pub struct Runner<'tape, Ram, Env>
where
    Env: ?Sized,
{
    tape: &'tape [MaybeUninit<usize>],
    marker: marker<fn(&mut Ram, &mut Env)>,
    id: Id<'tape>,
}

impl<'tape, Ram, Env> Runner<'tape, Ram, Env> {
    #[inline(always)]
    pub fn resolve_offset(self, offset: Offset<'tape>) -> Addr<'tape> {
        debug_assert!(offset.value < self.tape.len().wrapping_mul(mem::size_of::<usize>()));
        unsafe {
            let byte = (self.tape.as_ptr() as *const u8).add(offset.value);
            Addr {
                token: &*(byte as *const _),
                id: offset.id,
            }
        }
    }

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

impl<'tape, Op> Deref for Pc<'tape, Op> {
    type Target = Op;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.instruction.op
    }
}

#[repr(transparent)]
pub struct Pc<'tape, Op> {
    instruction: &'tape Instruction<Op>,
    id: Id<'tape>,
}

impl<'tape, Op> Pc<'tape, Op> {
    #[inline(always)]
    pub fn current(self) -> Addr<'tape> {
        Addr {
            token: &self.instruction.token,
            id: self.id,
        }
    }

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

pub type Destination<'tape> = Result<Addr<'tape>, Halt<'tape>>;

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
