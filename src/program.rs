//! Programs and how to run them.

use crate::cpu::{Addr, Dispatch};
use crate::debug_info::{DebugInfo, Dumper};
use crate::Runner;
use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::MaybeUninit;

/// A compiled program.
pub struct Program<Cpu, Tape, Rom, Ram>
where
    Ram: ?Sized,
{
    cpu: Cpu,
    tape: Tape,
    debug_info: DebugInfo,
    rom: Rom,
    not_sync: marker<*mut ()>,
    marker: marker<fn(&mut Ram)>,
}

impl<Cpu, Tape, Rom, Ram> Program<Cpu, Tape, Rom, Ram>
where
    Ram: ?Sized,
{
    /// Runs the program in a given environment.
    pub fn run(&mut self, ram: &mut Ram)
    where
        Cpu: Dispatch<Ram>,
        Tape: AsRef<[MaybeUninit<usize>]>,
    {
        let tape = self.tape.as_ref();
        unsafe {
            let runner = Runner::new(tape);
            let addr = Addr {
                token: &*(tape.as_ptr() as *const _),
                id: runner.id,
            };
            self.cpu.dispatch(addr, runner, ram)
        }
    }

    /// Gets a reference to the ROM used by the program.
    #[inline(always)]
    pub fn rom(&self) -> &Rom {
        &self.rom
    }

    #[inline(always)]
    pub(crate) unsafe fn new(cpu: Cpu, tape: Tape, debug_info: DebugInfo, rom: Rom) -> Self {
        Self {
            cpu,
            tape,
            debug_info,
            rom,
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
            .field("rom", &self.rom)
            .finish()
    }
}
