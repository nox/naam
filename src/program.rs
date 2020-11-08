use crate::cpu::{Addr, Dispatch};
use crate::debug_info::{DebugInfo, Dumper};
use crate::id::Id;
use crate::Runner;
use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::MaybeUninit;

/// A compiled program.
pub struct Program<Cpu, Tape, Env, In>
where
    In: ?Sized,
{
    cpu: Cpu,
    tape: Tape,
    debug_info: DebugInfo,
    env: Env,
    not_sync: marker<*mut ()>,
    marker: marker<fn(&mut Env, &mut In)>,
}

impl<Cpu, Tape, Env, In> Program<Cpu, Tape, Env, In>
where
    In: ?Sized,
{
    #[inline(never)]
    pub fn run(&mut self, input: &mut In)
    where
        Cpu: Dispatch<Env, In>,
        Tape: AsRef<[MaybeUninit<usize>]>,
    {
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

    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.env
    }

    #[inline(always)]
    pub fn env_mut(&mut self) -> &mut Env {
        &mut self.env
    }

    #[inline(always)]
    pub(crate) unsafe fn new(cpu: Cpu, tape: Tape, debug_info: DebugInfo, env: Env) -> Self {
        Self {
            cpu,
            tape,
            debug_info,
            env,
            not_sync: marker,
            marker,
        }
    }
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
