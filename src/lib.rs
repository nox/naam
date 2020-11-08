//! **N**ox's **A**bstract **A**bstract **M**achine

#![cfg_attr(not(feature = "std"), no_std)]

pub mod builtins;
pub mod cpu;
pub mod debug_info;
mod id;
pub mod tape;

use crate::builtins::Unreachable;
use crate::cpu::{Dispatch, DispatchToken};
use crate::debug_info::{DebugInfo, Dump, Dumper};
use crate::id::Id;
use crate::tape::{AsClearedWriter, UnexpectedEndError, Writer};

use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::{self, MaybeUninit};
use core::ops::Deref;
use core::ptr;

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
        let mut builder = Builder {
            writer: self.tape.as_cleared_writer(),
            cpu: self.cpu,
            debug_info: DebugInfo::default(),
            id: Id::default(),
            marker,
        };
        build(&mut builder, &mut self.env)?;
        builder.write(Unreachable)?;
        let debug_info = builder.debug_info;
        Ok(Program {
            cpu: self.cpu,
            debug_info,
            tape: self.tape,
            env: self.env,
            marker,
        })
    }
}

/// A compiled program.
pub struct Program<Cpu, Tape, Env, In>
where
    In: ?Sized,
{
    cpu: Cpu,
    tape: Tape,
    debug_info: DebugInfo,
    env: Env,
    marker: marker<fn(&mut Env, &mut In)>,
}

impl<Cpu, Tape, Env, In> Program<Cpu, Tape, Env, In>
where
    In: ?Sized,
{
    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.env
    }

    #[inline(always)]
    pub fn env_mut(&mut self) -> &mut Env {
        &mut self.env
    }
}

pub struct Builder<'tape, Cpu, Env, In>
where
    In: ?Sized,
{
    writer: &'tape mut dyn Writer,
    cpu: Cpu,
    debug_info: DebugInfo,
    #[allow(dead_code)]
    id: Id<'tape>,
    marker: marker<fn(&mut Env, &mut In)>,
}

impl<'tape, Cpu, Env, In> Builder<'tape, Cpu, Env, In>
where
    Cpu: Dispatch<Env, In>,
    In: ?Sized,
{
    pub fn write<Op>(&mut self, op: Op) -> Result<(), UnexpectedEndError>
    where
        Op: Execute<'tape, Env, In>,
        In: 'tape,
    {
        let instruction = Instruction {
            token: self.cpu.get_dispatch_token::<Op>(),
            op,
        };

        if mem::align_of_val(&instruction) != mem::size_of::<usize>() {
            panic!("instruction is over-aligned");
        }

        let size_in_words = mem::size_of_val(&instruction) / mem::size_of::<usize>();
        #[cfg(feature = "std")]
        let offset = self.writer.offset();
        unsafe {
            let slice = self.writer.take(size_in_words)?;
            ptr::write(slice.as_mut_ptr() as *mut _, instruction);
            #[cfg(feature = "std")]
            self.debug_info.push::<Instruction<Op>>(offset);
        }
        Ok(())
    }

    pub fn offset(&self) -> Offset<'tape> {
        Offset {
            value: self.writer.offset(),
            id: Id::default(),
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
    ) -> Result<Addr<'tape>, Halt>;
}

pub struct Halt;

impl<Cpu, Tape, Env, In> Program<Cpu, Tape, Env, In>
where
    Cpu: Dispatch<Env, In>,
    Tape: AsRef<[MaybeUninit<usize>]>,
{
    #[inline(never)]
    pub fn run(&mut self, input: &mut In) {
        let tape = self.tape.as_ref();
        unsafe {
            let runner = Runner {
                tape,
                marker: self.marker,
                id: Id::default(),
            };
            let addr = Addr {
                token: &*(tape.as_ptr() as *const _),
                id: runner.id,
            };
            self.cpu.dispatch(addr, runner, &mut self.env, input)
        }
    }
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
    pub fn resolve_offset(&self, offset: Offset<'tape>) -> Addr<'tape> {
        debug_assert!(offset.value < self.tape.len());
        unsafe {
            let word = self.tape.get_unchecked(offset.value);
            Addr {
                token: &*(word as *const _ as *const _),
                id: offset.id,
            }
        }
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
        unsafe {
            Addr {
                token: &*(self.instruction as *const _ as *const _),
                id: self.id,
            }
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

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Addr<'tape> {
    token: &'tape DispatchToken,
    id: Id<'tape>,
}

impl<'tape> Addr<'tape> {
    #[inline(always)]
    fn token(self) -> DispatchToken {
        *self.token
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Offset<'tape> {
    value: usize,
    id: Id<'tape>,
}

impl<Cpu, Tape, Env, In> fmt::Debug for Program<Cpu, Tape, Env, In>
where
    Cpu: Debug,
    Env: Debug,
    Tape: AsRef<[MaybeUninit<usize>]>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let dumper = unsafe { Dumper::new(self.tape.as_ref()) };
        fmt.debug_struct("Machine")
            .field("cpu", &self.cpu)
            .field("env", &self.env)
            .field("tape", &dumper.debug(&self.debug_info))
            .finish()
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Instruction<Op> {
    token: DispatchToken,
    op: Op,
}
