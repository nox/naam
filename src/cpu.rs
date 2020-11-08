use crate::builtins;
use crate::id::Id;
use crate::{Destination, Execute, Pc, Runner};
use core::fmt;
use core::mem;

pub trait GetDispatchToken<'tape, Op, Env, In>: Copy
where
    Op: Execute<'tape, Env, In>,
    In: ?Sized,
{
    fn get_dispatch_token(self) -> DispatchToken;
}

pub trait Dispatch<Env, In>: Copy + for<'tape> Unreachable<'tape, Env, In>
where
    for<'tape> Self: GetDispatchToken<'tape, <Self as Unreachable<'tape, Env, In>>::Op, Env, In>,
    In: ?Sized,
{
    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Env, In>,
        env: &mut Env,
        input: &mut In,
    );
}

pub unsafe trait Unreachable<'tape, Env, In>
where
    In: ?Sized,
{
    type Op: Execute<'tape, Env, In>;

    fn unreachable(&self) -> Self::Op;
}

#[derive(Clone, Copy)]
pub struct DispatchToken(usize);

impl From<usize> for DispatchToken {
    #[inline(always)]
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<DispatchToken> for usize {
    #[inline(always)]
    fn from(token: DispatchToken) -> Self {
        token.0
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Addr<'tape> {
    pub(crate) token: &'tape DispatchToken,
    pub(crate) id: Id<'tape>,
}

impl<'tape> Addr<'tape> {
    #[inline(always)]
    pub fn token(self) -> DispatchToken {
        *self.token
    }
}

#[derive(Clone, Copy)]
pub struct Halt<'tape> {
    #[allow(dead_code)]
    pub(crate) id: Id<'tape>,
}

impl fmt::Debug for Halt<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("Halt")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DirectThreadedLoop;

impl<'tape, Op, Env, In> GetDispatchToken<'tape, Op, Env, In> for DirectThreadedLoop
where
    Op: Execute<'tape, Env, In>,
    In: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token(self) -> DispatchToken {
        unsafe fn exec<'tape, Op, Env, In>(
            addr: Addr<'tape>,
            runner: Runner<'tape, Env, In>,
            env: &mut Env,
            input: &mut In,
        ) -> Destination<'tape>
        where
            Op: Execute<'tape, Env, In>,
            In: ?Sized,
        {
            Op::execute(Pc::from_addr(addr), runner, env, input)
        }

        DispatchToken::from(
            exec::<Op, Env, In> as OpaqueExec<'tape, Env, In, Destination<'tape>> as usize,
        )
    }
}

impl<Env, In> Dispatch<Env, In> for DirectThreadedLoop
where
    In: ?Sized,
{
    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        mut addr: Addr<'tape>,
        runner: Runner<'tape, Env, In>,
        env: &mut Env,
        input: &mut In,
    ) {
        loop {
            let function = mem::transmute::<usize, OpaqueExec<'tape, Env, In, Destination<'tape>>>(
                addr.token().into(),
            );
            match function(addr, runner, env, input) {
                Ok(next) => addr = next,
                Err(_) => return,
            }
        }
    }
}

unsafe impl<'tape, Env, In> Unreachable<'tape, Env, In> for DirectThreadedLoop
where
    In: ?Sized,
{
    type Op = builtins::Unreachable;

    #[inline(always)]
    fn unreachable(&self) -> Self::Op {
        builtins::Unreachable
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DirectThreadedCall;

impl<'tape, Op, Env, In> GetDispatchToken<'tape, Op, Env, In> for DirectThreadedCall
where
    Op: Execute<'tape, Env, In>,
    In: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token(self) -> DispatchToken {
        unsafe fn exec<'tape, Op, Env, In>(
            addr: Addr<'tape>,
            runner: Runner<'tape, Env, In>,
            env: &mut Env,
            input: &mut In,
        ) where
            Op: Execute<'tape, Env, In>,
            In: ?Sized,
        {
            match Op::execute(Pc::from_addr(addr), runner, env, input) {
                Ok(addr) => DirectThreadedCall.dispatch(addr, runner, env, input),
                Err(_) => (),
            }
        }

        DispatchToken::from(exec::<Op, Env, In> as OpaqueExec<'tape, Env, In, ()> as usize)
    }
}

impl<Env, In> Dispatch<Env, In> for DirectThreadedCall
where
    In: ?Sized,
{
    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Env, In>,
        env: &mut Env,
        input: &mut In,
    ) {
        let function = mem::transmute::<usize, OpaqueExec<'tape, Env, In, ()>>(addr.token().into());
        function(addr, runner, env, input)
    }
}

unsafe impl<'tape, Env, In> Unreachable<'tape, Env, In> for DirectThreadedCall
where
    In: ?Sized,
{
    type Op = builtins::Unreachable;

    #[inline(always)]
    fn unreachable(&self) -> Self::Op {
        builtins::Unreachable
    }
}

type OpaqueExec<'tape, Env, In, Out> =
    unsafe fn(Addr<'tape>, Runner<'tape, Env, In>, &mut Env, &mut In) -> Out;
