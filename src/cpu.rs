use crate::builtins;
use crate::id::Id;
use crate::{Destination, Execute, Pc, Runner};
use core::fmt;
use core::mem;

pub trait GetDispatchToken<'tape, Op, Ram, Env>: Copy
where
    Op: Execute<'tape, Ram, Env>,
    Env: ?Sized,
{
    fn get_dispatch_token(self) -> DispatchToken;
}

pub trait Dispatch<Ram, Env>: Copy + for<'tape> Unreachable<'tape, Ram, Env>
where
    for<'tape> Self: GetDispatchToken<'tape, <Self as Unreachable<'tape, Ram, Env>>::Op, Ram, Env>,
    Env: ?Sized,
{
    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    );
}

pub unsafe trait Unreachable<'tape, Ram, Env>
where
    Env: ?Sized,
{
    type Op: Execute<'tape, Ram, Env>;

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

impl<'tape, Op, Ram, Env> GetDispatchToken<'tape, Op, Ram, Env> for DirectThreadedLoop
where
    Op: Execute<'tape, Ram, Env>,
    Env: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token(self) -> DispatchToken {
        unsafe fn exec<'tape, Op, Ram, Env>(
            addr: Addr<'tape>,
            runner: Runner<'tape, Ram, Env>,
            ram: &mut Ram,
            env: &mut Env,
        ) -> Destination<'tape>
        where
            Op: Execute<'tape, Ram, Env>,
            Env: ?Sized,
        {
            Op::execute(Pc::from_addr(addr), runner, ram, env)
        }

        DispatchToken::from(
            exec::<Op, Ram, Env> as OpaqueExec<'tape, Ram, Env, Destination<'tape>> as usize,
        )
    }
}

impl<Ram, Env> Dispatch<Ram, Env> for DirectThreadedLoop
where
    Env: ?Sized,
{
    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        mut addr: Addr<'tape>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    ) {
        loop {
            let function = mem::transmute::<usize, OpaqueExec<'tape, Ram, Env, Destination<'tape>>>(
                addr.token().into(),
            );
            match function(addr, runner, ram, env) {
                Ok(next) => addr = next,
                Err(_) => return,
            }
        }
    }
}

unsafe impl<'tape, Ram, Env> Unreachable<'tape, Ram, Env> for DirectThreadedLoop
where
    Env: ?Sized,
{
    type Op = builtins::Unreachable;

    #[inline(always)]
    fn unreachable(&self) -> Self::Op {
        builtins::Unreachable
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DirectThreadedCall;

impl<'tape, Op, Ram, Env> GetDispatchToken<'tape, Op, Ram, Env> for DirectThreadedCall
where
    Op: Execute<'tape, Ram, Env>,
    Env: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token(self) -> DispatchToken {
        unsafe fn exec<'tape, Op, Ram, Env>(
            addr: Addr<'tape>,
            runner: Runner<'tape, Ram, Env>,
            ram: &mut Ram,
            env: &mut Env,
        ) where
            Op: Execute<'tape, Ram, Env>,
            Env: ?Sized,
        {
            match Op::execute(Pc::from_addr(addr), runner, ram, env) {
                Ok(addr) => DirectThreadedCall.dispatch(addr, runner, ram, env),
                Err(_) => (),
            }
        }

        DispatchToken::from(exec::<Op, Ram, Env> as OpaqueExec<'tape, Ram, Env, ()> as usize)
    }
}

impl<Ram, Env> Dispatch<Ram, Env> for DirectThreadedCall
where
    Env: ?Sized,
{
    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    ) {
        let function = mem::transmute::<usize, OpaqueExec<'tape, Ram, Env, ()>>(addr.token().into());
        function(addr, runner, ram, env)
    }
}

unsafe impl<'tape, Ram, Env> Unreachable<'tape, Ram, Env> for DirectThreadedCall
where
    Env: ?Sized,
{
    type Op = builtins::Unreachable;

    #[inline(always)]
    fn unreachable(&self) -> Self::Op {
        builtins::Unreachable
    }
}

type OpaqueExec<'tape, Ram, Env, Out> =
    unsafe fn(Addr<'tape>, Runner<'tape, Ram, Env>, &mut Ram, &mut Env) -> Out;
