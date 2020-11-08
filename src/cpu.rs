use crate::{Addr, Execute, Halt, OpaquePc, Pc, Runner};

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
        DispatchToken::from(Op::execute as *const () as usize)
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
            let pc = OpaquePc::from_addr(addr);
            let function = mem::transmute::<
                usize,
                fn(
                    OpaquePc<'tape>,
                    Runner<'tape, Env, In>,
                    &mut Env,
                    &mut In,
                ) -> Result<Addr<'tape>, Halt>,
            >(pc.token().into());
            match function(pc, runner, env, input) {
                Ok(next) => addr = next,
                Err(Halt) => return,
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
            pc: Pc<'tape, Op>,
            runner: Runner<'tape, Env, In>,
            env: &mut Env,
            input: &mut In,
        ) where
            Op: Execute<'tape, Env, In>,
            In: ?Sized,
        {
            match Op::execute(pc, runner, env, input) {
                Ok(addr) => DirectThreadedCall.dispatch(addr, runner, env, input),
                Err(Halt) => (),
            }
        }

        DispatchToken::from(exec::<Op, Env, In> as *const () as usize)
    }

    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Env, In>,
        env: &mut Env,
        input: &mut In,
    ) {
        let pc = OpaquePc::from_addr(addr);
        let function = mem::transmute::<
            usize,
            fn(OpaquePc<'tape>, Runner<'tape, Env, In>, &mut Env, &mut In),
        >(pc.token().into());
        function(pc, runner, env, input)
    }
}
