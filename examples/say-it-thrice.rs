#![forbid(unsafe_code)]

extern crate naam;

use naam::cpu::DirectThreadedLoop as Cpu;
use naam::{Addr, Execute, Halt, Machine, Offset, Pc, Runner};
use std::any;
use std::fmt::Debug;

fn main() {
    let machine = Machine::new(Cpu, vec![], 0);
    let mut program = machine
        .program(|builder, _env| {
            let print_hello_world = builder.offset();
            builder.write(PrintLn("Hello, world!"))?;
            builder.write(JumpNTimes(print_hello_world))?;
            builder.write(Return(42))
        })
        .unwrap();
    println!("{:#?}\n", program);
    program.run(&mut 2);
    assert!(*program.env() == 42);
}

#[derive(Clone, Copy, Debug)]
struct Return<Out>(Out);

impl<'tape, In, Out> Execute<'tape, Out, In> for Return<Out>
where
    Out: 'tape + Copy + Debug,
{
    fn execute(
        pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Out, In>,
        env: &mut Out,
        _input: &mut In,
    ) -> Result<Addr<'tape>, Halt> {
        *env = pc.0;
        Err(Halt)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
struct PrintLn(&'static str);

impl<'tape, Env, In> Execute<'tape, Env, In> for PrintLn {
    #[inline(always)]
    fn execute(
        pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Env, In>,
        _env: &mut Env,
        _in: &mut In,
    ) -> Result<Addr<'tape>, Halt> {
        println!("{}", pc.0);
        Ok(pc.next())
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct JumpNTimes<'tape>(Offset<'tape>);

impl<'tape, Env> Execute<'tape, Env, usize> for JumpNTimes<'tape> {
    fn execute(
        pc: Pc<'tape, Self>,
        runner: Runner<'tape, Env, usize>,
        _env: &mut Env,
        input: &mut usize,
    ) -> Result<Addr<'tape>, Halt> {
        Ok(if *input > 0 {
            *input -= 1;
            runner.resolve_offset(pc.0)
        } else {
            pc.next()
        })
    }
}

mod should_be_derived {
    use super::*;
    use naam::debug_info::{Dump, Dumper};
    use std::fmt::{self, Debug};

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
