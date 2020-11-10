//! Programs and how to run them.

use crate::builder::Builder;
use crate::builtins::Unreachable;
use crate::code::Build;
use crate::cpu::{Addr, Dispatch};
use crate::debug_info::{DebugInfo, Dumper};
use crate::Runner;
use crate::tape::AsClearedWriter;
use core::fmt::{self, Debug};
use core::marker::PhantomData as marker;
use core::mem::MaybeUninit;
use core::ops::Deref;
use stable_deref_trait::StableDeref;

/// A compiled program.
pub struct Program<Cpu, Tape, Code, Ram>
where
    Ram: ?Sized,
{
    cpu: Cpu,
    tape: Tape,
    debug_info: DebugInfo,
    code: Code,
    not_sync: marker<*mut ()>,
    marker: marker<fn(&mut Ram)>,
}

impl<Cpu, Tape, Code, Ram> Program<Cpu, Tape, Code, Ram>
where
    Ram: ?Sized,
{
    /// Returns a new program built from the given code.
    pub fn new(
        cpu: Cpu,
        mut tape: Tape,
        code: Code,
    ) -> Result<Program<Cpu, Tape, Code, Ram>, <<Code as Deref>::Target as Build<Cpu, Ram>>::Error>
    where
        Cpu: Dispatch<Ram>,
        Tape: AsClearedWriter,
        Code: StableDeref,
        <Code as Deref>::Target: Build<Cpu, Ram>,
    {
        let mut builder = Builder::new(cpu, &mut tape);
        code.deref().build(&mut builder)?;
        builder.emit(Unreachable)?;
        unsafe {
            let debug_info = builder.into_debug_info();
            Ok(Self {
                cpu,
                tape,
                debug_info,
                code,
                not_sync: marker,
                marker,
            })
        }
    }

    /// Runs the program in a given environment.
    pub fn run(&self, ram: &mut Ram)
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

    /// Gets a reference to the code used by the program.
    #[inline(always)]
    pub fn code(&self) -> &Code {
        &self.code
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
            .field("code", &self.code)
            .field("tape", &dumper.debug(&self.debug_info))
            .finish()
    }
}
