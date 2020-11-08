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
pub struct Machine<Cpu, Tape, Env> {
    cpu: Cpu,
    tape: Tape,
    env: Env,
}

impl<Cpu, Tape, Env> Machine<Cpu, Tape, Env> {
    #[inline(always)]
    pub fn new(cpu: Cpu, tape: Tape, env: Env) -> Self {
        Self { cpu, tape, env }
    }

    pub fn program<In, Error>(
        mut self,
        build: impl FnOnce(&mut Builder<'_, Cpu, Env, In>, &mut Env) -> Result<(), Error>,
    ) -> Result<Program<Cpu, Tape, Env, In>, Error>
    where
        Cpu: Dispatch<Env, In>,
        Tape: AsClearedWriter,
        Error: From<UnexpectedEndError>,
    {
        let mut builder = Builder::new(self.cpu, &mut self.tape);
        build(&mut builder, &mut self.env)?;
        builder.emit(self.cpu.unreachable())?;
        unsafe {
            let debug_info = builder.into_debug_info();
            Ok(Program::new(self.cpu, self.tape, debug_info, self.env))
        }
    }
}

pub trait Execute<'tape, Env, In>: 'tape + Copy + Dump<'tape> + Sized
where
    In: ?Sized,
{
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape, Env, In>,
        env: &mut Env,
        input: &mut In,
    ) -> Destination<'tape>;
}

#[repr(transparent)]
pub struct Runner<'tape, Env, In>
where
    In: ?Sized,
{
    tape: &'tape [MaybeUninit<usize>],
    marker: marker<fn(&mut Env, &mut In)>,
    id: Id<'tape>,
}

impl<'tape, Env, In> Runner<'tape, Env, In> {
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
}

impl<'tape, Env, In> Clone for Runner<'tape, Env, In>
where
    In: ?Sized,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<'tape, Env, In> Copy for Runner<'tape, Env, In> where In: ?Sized {}

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
