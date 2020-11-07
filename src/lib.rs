//! **N**ox's **A**bstract **A**bstract **M**achine

#![cfg_attr(not(feature = "std"), no_std)]

pub mod debug_info;
mod id;
pub mod tape;

use crate::debug_info::{DebugInfo, Dump, Dumper};
use crate::id::Id;
use crate::tape::{AsClearedWriter, UnexpectedEndError, Writer};

use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::{self, MaybeUninit};
use core::ops::Deref;
use core::ptr;

/// A compiled program.
pub struct Program<Env, Tape, In>
where
    In: ?Sized,
{
    env: Env,
    tape: Tape,
    debug_info: DebugInfo,
    marker: marker<fn(&mut Env, &mut In)>,
}

impl<Env, Tape, In> Program<Env, Tape, In>
where
    In: ?Sized,
{
    pub fn new<Error>(
        mut env: Env,
        mut tape: Tape,
        build: impl FnOnce(&mut Builder<'_, Env, In>, &mut Env) -> Result<(), Error>,
    ) -> Result<Self, Error>
    where
        Tape: AsClearedWriter,
        Error: From<UnexpectedEndError>,
    {
        let mut builder = Builder {
            writer: tape.as_cleared_writer(),
            debug_info: DebugInfo::default(),
            id: Id::default(),
            marker,
        };
        build(&mut builder, &mut env)?;
        builder.write(Unreachable)?;
        let debug_info = builder.debug_info;
        Ok(Self {
            env,
            debug_info,
            tape,
            marker,
        })
    }

    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.env
    }

    #[inline(always)]
    pub fn env_mut(&mut self) -> &mut Env {
        &mut self.env
    }
}

pub struct Builder<'tape, Env, In>
where
    In: ?Sized,
{
    writer: &'tape mut dyn Writer,
    debug_info: DebugInfo,
    #[allow(dead_code)]
    id: Id<'tape>,
    marker: marker<fn(&mut Env, &mut In)>,
}

impl<'tape, Env, In> Builder<'tape, Env, In>
where
    In: ?Sized,
{
    pub fn write<Op>(&mut self, operands: Op) -> Result<(), UnexpectedEndError>
    where
        Op: Execute<'tape, Env, In>,
        In: 'tape,
    {
        let instruction = Instruction {
            function: OpaqueFunction::for_op::<Op, _, _>(),
            operands,
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

impl<Env, Tape, In> Program<Env, Tape, In>
where
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
            let mut addr = Addr {
                instruction: &*(tape.as_ptr() as *const _),
                id: runner.id,
            };
            loop {
                let pc = Pc {
                    instruction: addr.instruction,
                    id: addr.id,
                };
                match pc.instruction.function.as_dummy()(pc, runner, &mut self.env, input) {
                    Ok(next) => addr = next,
                    Err(Halt) => return,
                }
            }
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
                instruction: &*(word as *const _ as *const _),
                id: offset.id,
            }
        }
    }
}

impl<'tape, Env, In> Clone for Runner<'tape, Env, In> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<'tape, Env, In> Copy for Runner<'tape, Env, In> {}

impl<'tape, Op> Deref for Pc<'tape, Op> {
    type Target = Op;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.instruction.operands
    }
}

impl<'tape, Op> Pc<'tape, Op> {
    #[inline(always)]
    pub fn current(self) -> Addr<'tape> {
        unsafe {
            Addr {
                instruction: &*(self.instruction as *const _ as *const _),
                id: self.id,
            }
        }
    }

    #[inline(always)]
    pub fn next(self) -> Addr<'tape> {
        unsafe {
            let end = (self.instruction as *const Instruction<Op>).add(1);
            Addr {
                instruction: &*(end as *const _),
                id: self.id,
            }
        }
    }
}

#[repr(transparent)]
pub struct Pc<'tape, Op> {
    instruction: &'tape Instruction<Op>,
    id: Id<'tape>,
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
    instruction: &'tape Instruction<DummyOp>,
    id: Id<'tape>,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Offset<'tape> {
    value: usize,
    id: Id<'tape>,
}

impl<Env, Tape, In> fmt::Debug for Program<Env, Tape, In>
where
    Env: Debug,
    Tape: AsRef<[MaybeUninit<usize>]>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let dumper = unsafe { Dumper::new(self.tape.as_ref()) };
        fmt.debug_struct("Machine")
            .field("env", &self.env)
            .field("tape", &dumper.debug(&self.debug_info))
            .finish()
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Instruction<Op> {
    function: OpaqueFunction,
    operands: Op,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct OpaqueFunction(*const ());

impl OpaqueFunction {
    fn for_op<'tape, Op, Env, In>() -> Self
    where
        Op: Execute<'tape, Env, In>,
        In: ?Sized,
    {
        Self(Op::execute as *const ())
    }

    #[inline(always)]
    unsafe fn as_dummy<'tape, Env, In>(
        self,
    ) -> fn(
        Pc<'tape, DummyOp>,
        Runner<'tape, Env, In>,
        &mut Env,
        &mut In,
    ) -> Result<Addr<'tape>, Halt> {
        mem::transmute(self.0)
    }
}

struct DummyOp;

#[derive(Clone, Copy, Debug)]
struct Unreachable;

impl<'tape> Dump<'tape> for Unreachable {
    fn dump(&self, fmt: &mut fmt::Formatter, _dumper: &Dumper<'tape>) -> fmt::Result {
        self.fmt(fmt)
    }
}

impl<'tape, Env, In> Execute<'tape, Env, In> for Unreachable
where
    In: ?Sized,
{
    fn execute(
        _pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Env, In>,
        _env: &mut Env,
        _input: &mut In,
    ) -> Result<Addr<'tape>, Halt> {
        panic!("reached unreachable tape")
    }
}
