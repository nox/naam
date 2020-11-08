use crate::cpu::Dispatch;
use crate::debug_info::DebugInfo;
use crate::id::Id;
use crate::tape::{AsClearedWriter, UnexpectedEndError, Writer};
use crate::{Execute, Instruction, Offset};
use core::marker::PhantomData as marker;
use core::mem;
use core::ptr;

pub struct Builder<'tape, Cpu, Env, In>
where
    In: ?Sized,
{
    cpu: Cpu,
    writer: &'tape mut dyn Writer,
    debug_info: DebugInfo,
    #[allow(dead_code)]
    id: Id<'tape>,
    marker: marker<fn(&mut Env, &mut In)>,
}

impl<'tape, Cpu, Env, In> Builder<'tape, Cpu, Env, In>
where
    Cpu: Dispatch<Env, In>,
    In: ?Sized,
{
    pub fn emit<Op>(&mut self, op: Op) -> Result<(), UnexpectedEndError>
    where
        Op: Execute<'tape, Env, In>,
        In: 'tape,
    {
        let instruction = Instruction {
            token: self.cpu.get_dispatch_token::<Op>(),
            op,
        };

        if mem::align_of_val(&instruction) != mem::size_of::<usize>() {
            panic!("instruction is over-aligned");
        }

        let size_in_words = mem::size_of_val(&instruction) / mem::size_of::<usize>();
        #[cfg(feature = "std")]
        let offset = self.writer.offset();
        unsafe {
            let slice = self.writer.take(size_in_words)?;
            ptr::write(slice.as_mut_ptr() as *mut _, instruction);
            #[cfg(feature = "std")]
            self.debug_info.push::<Instruction<Op>>(offset);
        }
        Ok(())
    }

    #[inline(always)]
    pub fn offset(&self) -> Offset<'tape> {
        Offset {
            value: self.writer.offset(),
            id: Id::default(),
        }
    }

    #[inline(always)]
    pub(crate) fn new<Tape>(cpu: Cpu, tape: &'tape mut Tape) -> Self
    where
        Tape: AsClearedWriter,
    {
        Self {
            writer: tape.as_cleared_writer(),
            cpu,
            debug_info: DebugInfo::default(),
            id: Id::default(),
            marker,
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn into_debug_info(self) -> DebugInfo {
        self.debug_info
    }
}
