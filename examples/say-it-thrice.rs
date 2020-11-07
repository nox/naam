#![forbid(unsafe_code)]

extern crate naam;

use naam::debug_info::{Dump, Dumper};
use naam::{Addr, Execute, Halt, Offset, Pc, Program, Runner};
use std::fmt::{self, Debug};

fn main() {
    let tape = vec![];
    let mut program = Program::new(Env(42), tape, |writer, _env| {
        let print_hello_world = writer.offset();
        writer.write(PrintLn("Hello, world!"))?;
        writer.write(JumpNTimes(print_hello_world))?;
        writer.write(Return(42))
    })
    .unwrap();
    println!("{:#?}\n", program);
    program.run(&mut 2);
    assert!(program.env().0 == 42);
}

#[derive(Debug)]
struct Env<Out>(Out);

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
struct Return<Out>(Out);

impl<'tape, In, Out> Execute<'tape, Env<Out>, In> for Return<Out>
where
    Out: 'tape + Copy + Debug,
{
    fn execute(
        pc: Pc<'tape, Self>,
        _runner: Runner<'tape, Env<Out>, In>,
        env: &mut Env<Out>,
        _input: &mut In,
    ) -> Result<Addr<'tape>, Halt> {
        env.0 = pc.0;
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

// This is code that should be derived, not written by hand.

impl<'tape, Out> Dump<'tape> for Return<Out>
where
    Out: Debug,
{
    fn dump(&self, fmt: &mut fmt::Formatter, _dumper: &Dumper<'tape>) -> fmt::Result {
        self.fmt(fmt)
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
