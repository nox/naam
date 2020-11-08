use crate::cpu::{Addr, Dispatch};
use crate::debug_info::{DebugInfo, Dumper};
use crate::Runner;
use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::MaybeUninit;

/// A compiled program.
pub struct Program<Cpu, Tape, Ram, Env>
where
    Env: ?Sized,
{
    cpu: Cpu,
    tape: Tape,
    debug_info: DebugInfo,
    ram: Ram,
    not_sync: marker<*mut ()>,
    marker: marker<fn(&mut Ram, &mut Env)>,
}

impl<Cpu, Tape, Ram, Env> Program<Cpu, Tape, Ram, Env>
where
    Env: ?Sized,
{
    #[inline(never)]
    pub fn run(&mut self, env: &mut Env)
    where
        Cpu: Dispatch<Ram, Env>,
        Tape: AsRef<[MaybeUninit<usize>]>,
    {
        let tape = self.tape.as_ref();
        unsafe {
            let runner = Runner::new(tape);
            let addr = Addr {
                token: &*(tape.as_ptr() as *const _),
                id: runner.id,
            };
            self.cpu.dispatch(addr, runner, &mut self.ram, env)
        }
    }

    #[inline(always)]
    pub fn ram(&self) -> &Ram {
        &self.ram
    }

    #[inline(always)]
    pub fn env_mut(&mut self) -> &mut Ram {
        &mut self.ram
    }

    #[inline(always)]
    pub(crate) unsafe fn new(cpu: Cpu, tape: Tape, debug_info: DebugInfo, ram: Ram) -> Self {
        Self {
            cpu,
            tape,
            debug_info,
            ram,
            not_sync: marker,
            marker,
        }
    }
}

impl<Cpu, Tape, Ram, Env> fmt::Debug for Program<Cpu, Tape, Ram, Env>
where
    Cpu: Debug,
    Ram: Debug,
    Tape: AsRef<[MaybeUninit<usize>]>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let dumper = unsafe { Dumper::new(self.tape.as_ref()) };
        fmt.debug_struct("Machine")
            .field("cpu", &self.cpu)
            .field("tape", &dumper.debug(&self.debug_info))
            .field("ram", &self.ram)
            .finish()
    }
}
