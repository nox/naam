#![forbid(unsafe_code)]

extern crate naam;

use naam::builder::{Build, Builder};
use naam::builtins::Nop;
use naam::cpu::DirectThreadedLoop as Cpu;
use naam::tape::UnexpectedEndError;
use naam::{Destination, Execute, Offset, Pc, Program, Runner};
use std::fmt::Debug;

fn main() {
    let hello = "Hello, world!".to_owned();
    let code = SayItNTimes(&hello);
    let program = Program::new(Cpu, vec![], &code).unwrap();
    println!("{:#?}\n", program);
    let mut ram = SayItNTimesRam {
        rval: 0,
        counter: 2,
    };
    program.run(&mut ram);
    assert!(ram.rval == 42);
}

#[derive(Debug)]
struct SayItNTimes<'a>(&'a str);

impl<'a> Build<Cpu> for SayItNTimes<'a> {
    type Ram = SayItNTimesRam;
    type Error = UnexpectedEndError;

    fn build<'tape, 'code>(
        &'code self,
        builder: &mut Builder<'tape, 'code, Cpu, SayItNTimesRam>,
    ) -> Result<(), Self::Error>
    where
        'code: 'tape,
    {
        builder.emit(Nop)?;
        let print_hello_world = builder.offset();
        builder.emit(PrintLn(self.0))?;
        builder.emit(JumpNTimes(print_hello_world))?;
        builder.emit(Return(42))
    }
}

#[derive(Clone, Copy, Debug)]
struct SayItNTimesRam {
    rval: usize,
    counter: usize,
}

#[derive(Clone, Copy, Debug)]
struct Return(usize);

impl<'tape> Execute<'tape, SayItNTimesRam> for Return {
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape>,
        ram: &mut SayItNTimesRam,
    ) -> Destination<'tape> {
        ram.rval = pc.0;
        Err(runner.halt())
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
struct PrintLn<'code>(&'code str);

impl<'tape, 'code: 'tape, Ram> Execute<'tape, Ram> for PrintLn<'code>
where
    Ram: ?Sized,
{
    #[inline(always)]
    fn execute(pc: Pc<'tape, Self>, _runner: Runner<'tape>, _ram: &mut Ram) -> Destination<'tape> {
        println!("{}", pc.0);
        Ok(pc.next())
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct JumpNTimes<'tape>(Offset<'tape>);

impl<'tape, 'code> Execute<'tape, SayItNTimesRam> for JumpNTimes<'tape> {
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape>,
        ram: &mut SayItNTimesRam,
    ) -> Destination<'tape> {
        Ok(if ram.counter > 0 {
            ram.counter -= 1;
            runner.resolve_offset(pc.0)
        } else {
            pc.next()
        })
    }
}

mod should_be_derived {
    use super::*;
    use core::fmt::{self, Debug};
    use naam::debug_info::{Dump, Dumper};

    impl<'tape> Dump<'tape> for Return {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: Dumper<'tape>) -> fmt::Result {
            self.fmt(fmt)
        }
    }

    impl<'tape> Dump<'tape> for PrintLn<'_> {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: Dumper<'tape>) -> fmt::Result {
            self.fmt(fmt)
        }
    }

    impl<'tape> Dump<'tape> for JumpNTimes<'tape> {
        fn dump(&self, fmt: &mut fmt::Formatter, dumper: Dumper<'tape>) -> fmt::Result {
            fmt.debug_tuple("JumpNTimes")
                .field(&dumper.debug(&self.0))
                .finish()
        }
    }
}
