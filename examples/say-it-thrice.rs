#![forbid(unsafe_code)]

extern crate naam;

use naam::builtins::Nop;
use naam::cpu::DirectThreadedLoop as Cpu;
use naam::{Destination, Execute, Machine, Offset, Pc, Runner};
use std::fmt::Debug;

fn main() {
    let machine = Machine::new(Cpu, vec![], 0);
    let mut program = machine
        .program(|builder, _env| {
            builder.emit(Nop)?;
            let print_hello_world = builder.offset();
            builder.emit(PrintLn("Hello, world!"))?;
            builder.emit(JumpNTimes(print_hello_world))?;
            builder.emit(Return(42))
        })
        .unwrap();
    println!("{:#?}\n", program);
    program.run(&mut 2);
    assert!(*program.ram() == 42);
}

#[derive(Clone, Copy, Debug)]
struct Return<Out>(Out);

impl<'tape, Env, Out> Execute<'tape, Out, Env> for Return<Out>
where
    Out: 'tape + Copy + Debug,
{
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape, Out, Env>,
        ram: &mut Out,
        _env: &mut Env,
    ) -> Destination<'tape> {
        *ram = pc.0;
        Err(runner.halt())
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
struct PrintLn(&'static str);

impl<'tape, Ram, Env> Execute<'tape, Ram, Env> for PrintLn {
    #[inline(always)]
    fn execute(
        pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Ram, Env>,
        _ram: &mut Ram,
        _env: &mut Env,
    ) -> Destination<'tape> {
        println!("{}", pc.0);
        Ok(pc.next())
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct JumpNTimes<'tape>(Offset<'tape>);

impl<'tape, Ram> Execute<'tape, Ram, usize> for JumpNTimes<'tape> {
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape, Ram, usize>,
        _ram: &mut Ram,
        env: &mut usize,
    ) -> Destination<'tape> {
        Ok(if *env > 0 {
            *env -= 1;
            runner.resolve_offset(pc.0)
        } else {
            pc.next()
        })
    }
}

mod should_be_derived {
    use super::*;
    use core::any;
    use core::fmt::{self, Debug};
    use naam::debug_info::{Dump, Dumper};

    impl<'tape, Out> Dump<'tape> for Return<Out>
    where
        Out: Debug,
    {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: &Dumper<'tape>) -> fmt::Result {
            write!(fmt, "Return<{}", any::type_name::<Out>())?;
            fmt.debug_tuple(">").field(&self.0).finish()
        }
    }

    impl<'tape> Dump<'tape> for PrintLn {
        fn dump(&self, fmt: &mut fmt::Formatter, _dumper: &Dumper<'tape>) -> fmt::Result {
            self.fmt(fmt)
        }
    }

    impl<'tape> Dump<'tape> for JumpNTimes<'tape> {
        fn dump(&self, fmt: &mut fmt::Formatter, dumper: &Dumper<'tape>) -> fmt::Result {
            fmt.debug_tuple("JumpNTimes")
                .field(&dumper.debug(&self.0))
                .finish()
        }
    }
}
