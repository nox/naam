//! CPU-related traits and a couple of built-in CPUs.

use crate::builtins;
use crate::id::Id;
use crate::{Destination, Execute, Pc, Runner};
use core::fmt;
use core::mem;

/// The main trait for CPUs.
///
/// A CPU dispatches operations based on which destination they return.
/// They are all equipped with a `Unreachable` implementation to emit
/// an operation that is guaranteed to panic, for safety reasons.
///
/// It is the CPU's responsibility to ensure the proper progression of the
/// program through the opaque `DispatchToken` values reachable from the
/// destinations returned by each operation, .
pub trait Dispatch<Ram, Env>: Copy + for<'tape> Unreachable<'tape, Ram, Env>
where
    for<'tape> Self: GetDispatchToken<'tape, <Self as Unreachable<'tape, Ram, Env>>::Op, Ram, Env>,
    Env: ?Sized,
{
    /// Dispatches the operation at the given address.
    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    );
}

/// CPUs should implement this trait for each operation they support.
///
/// **Note:** Implementors of this trait should also implement
/// `Dispatch<Ram, Env>`, but such a where clause would introduce a cycle
/// because of the `GetDispatchToken` bound in the definition of `Dispatch`.
pub unsafe trait GetDispatchToken<'tape, Op, Ram, Env>: Copy
where
    Op: Execute<'tape, Ram, Env>,
    Env: ?Sized,
{
    /// Returns the dispatch token for this operation.
    ///
    /// This dispatch token can be accessed from the address passed to
    /// `Self::dispatch`.
    fn get_dispatch_token(self) -> DispatchToken;
}

/// The unreachable operation supported by each CPU.
pub unsafe trait Unreachable<'tape, Ram, Env>
where
    Env: ?Sized,
{
    /// The type of the unreachable operation.
    type Op: Execute<'tape, Ram, Env>;

    /// Returns the unreachable operation.
    fn unreachable(&self) -> Self::Op;
}

/// An opaque dispatch token.
///
/// Opaque dispatch tokens can be converted to and from usize values through
/// the `From` trait.
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

/// An address into the tape.
///
/// Addresses are guaranteed to point at the start of an operation and are
/// thus always safe to dispatch to.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Addr<'tape> {
    pub(crate) token: &'tape DispatchToken,
    pub(crate) id: Id<'tape>,
}

impl<'tape> Addr<'tape> {
    /// Returns the dispatch token at this address.
    #[inline(always)]
    pub fn token(self) -> DispatchToken {
        *self.token
    }
}

/// Token to signal that the program should halt.
///
/// This is returned by `Runner::halt`.
#[derive(Clone, Copy)]
pub struct Halt<'tape> {
    #[allow(dead_code)]
    pub(crate) id: Id<'tape>,
}

impl fmt::Debug for Halt<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("Halt")
    }
}

/// A CPU that dispatches operations looping and calling operations directly.
///
/// This CPU supports all instructions.
///
/// **The term “direct-threaded” is a stretch here!** Instead of tail-calls like
/// `DirectThreadedLoop`, this CPU dispatches operations in a loop, so each
/// operation returns to the calling loop. It's only named that way because
/// it stores function pointers on the tape directly and doesn't use a jump
/// table.
#[derive(Clone, Copy, Debug)]
pub struct DirectThreadedLoop;

unsafe impl<'tape, Op, Ram, Env> GetDispatchToken<'tape, Op, Ram, Env> for DirectThreadedLoop
where
    Op: Execute<'tape, Ram, Env>,
    Env: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token(self) -> DispatchToken {
        // The dispatch token here is a function that returns a destination, as
        // Self.dispatch loops over return values from this function.
        unsafe fn exec<'tape, Op, Ram, Env>(
            addr: Addr<'tape>,
            runner: Runner<'tape, Ram, Env>,
            ram: &mut Ram,
            env: &mut Env,
        ) -> Destination<'tape>
        where
            Op: Execute<'tape, Ram, Env>,
            Env: ?Sized,
        {
            Op::execute(Pc::from_addr(addr), runner, ram, env)
        }

        DispatchToken::from(
            exec::<Op, Ram, Env> as OpaqueExec<'tape, Ram, Env, Destination<'tape>> as usize,
        )
    }
}

impl<Ram, Env> Dispatch<Ram, Env> for DirectThreadedLoop
where
    Env: ?Sized,
{
    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        mut addr: Addr<'tape>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    ) {
        loop {
            let function = mem::transmute::<usize, OpaqueExec<'tape, Ram, Env, Destination<'tape>>>(
                addr.token().into(),
            );
            match function(addr, runner, ram, env) {
                Ok(next) => addr = next,
                Err(_) => return,
            }
        }
    }
}

unsafe impl<'tape, Ram, Env> Unreachable<'tape, Ram, Env> for DirectThreadedLoop
where
    Env: ?Sized,
{
    type Op = builtins::Unreachable;

    #[inline(always)]
    fn unreachable(&self) -> Self::Op {
        builtins::Unreachable
    }
}

/// A CPU that dispatches operations the way a direct-threaded emulator does.
///
/// This CPU supports all instructions.
///
/// **Your stack may overflow if you use this.** This relies on the compiler
/// always doing a tail-call from one operation to the next one, but `rustc`
/// makes no guarantee it will, and it will do so only in release builds.
#[derive(Clone, Copy, Debug)]
pub struct DirectThreadedCall;

unsafe impl<'tape, Op, Ram, Env> GetDispatchToken<'tape, Op, Ram, Env> for DirectThreadedCall
where
    Op: Execute<'tape, Ram, Env>,
    Env: ?Sized,
{
    #[inline(always)]
    fn get_dispatch_token(self) -> DispatchToken {
        // The dispatch token here is a function that returns (), as it
        // calls Self.dispatch directly.
        unsafe fn exec<'tape, Op, Ram, Env>(
            addr: Addr<'tape>,
            runner: Runner<'tape, Ram, Env>,
            ram: &mut Ram,
            env: &mut Env,
        ) where
            Op: Execute<'tape, Ram, Env>,
            Env: ?Sized,
        {
            match Op::execute(Pc::from_addr(addr), runner, ram, env) {
                Ok(addr) => Self.dispatch(addr, runner, ram, env),
                Err(_) => (),
            }
        }

        DispatchToken::from(exec::<Op, Ram, Env> as OpaqueExec<'tape, Ram, Env, ()> as usize)
    }
}

impl<Ram, Env> Dispatch<Ram, Env> for DirectThreadedCall
where
    Env: ?Sized,
{
    #[inline(always)]
    unsafe fn dispatch<'tape>(
        self,
        addr: Addr<'tape>,
        runner: Runner<'tape, Ram, Env>,
        ram: &mut Ram,
        env: &mut Env,
    ) {
        let function =
            mem::transmute::<usize, OpaqueExec<'tape, Ram, Env, ()>>(addr.token().into());
        function(addr, runner, ram, env)
    }
}

unsafe impl<'tape, Ram, Env> Unreachable<'tape, Ram, Env> for DirectThreadedCall
where
    Env: ?Sized,
{
    type Op = builtins::Unreachable;

    #[inline(always)]
    fn unreachable(&self) -> Self::Op {
        builtins::Unreachable
    }
}

type OpaqueExec<'tape, Ram, Env, Out> =
    unsafe fn(Addr<'tape>, Runner<'tape, Ram, Env>, &mut Ram, &mut Env) -> Out;
