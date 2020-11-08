use crate::{Addr, Destination, Execute, Pc, Runner};

use core::mem;

pub trait Dispatch<Env, In>: Copy
where
    In: ?Sized,
{
    fn get_dispatch_token<'tape, Op>(self) -> DispatchToken
    where
        Op: Execute<'tape, Env, In>;

    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Env, In>,
        env: &mut Env,
        input: &mut In,
    );
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

#[derive(Clone, Copy, Debug)]
pub struct DirectThreadedLoop;

impl<Env, In> Dispatch<Env, In> for DirectThreadedLoop
where
    In: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token<'tape, Op>(self) -> DispatchToken
    where
        Op: Execute<'tape, Env, In>,
    {
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

    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        mut addr: Addr<'tape>,
        runner: Runner<'tape, Env, In>,
        env: &mut Env,
        input: &mut In,
    ) {
        loop {
            let function = mem::transmute::<
                usize,
                OpaqueExec<'tape, Env, In, Destination<'tape>>,
            >(addr.token().into());
            match function(addr, runner, env, input) {
                Ok(next) => addr = next,
                Err(_) => return,
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DirectThreadedCall;

impl<Env, In> Dispatch<Env, In> for DirectThreadedCall
where
    In: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token<'tape, Op>(self) -> DispatchToken
    where
        Op: Execute<'tape, Env, In>,
    {
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

type OpaqueExec<'tape, Env, In, Out> =
    unsafe fn(Addr<'tape>, Runner<'tape, Env, In>, &mut Env, &mut In) -> Out;
