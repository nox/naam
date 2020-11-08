use core::mem::MaybeUninit;

pub unsafe trait AsClearedWriter: AsRef<[MaybeUninit<usize>]> {
    fn as_cleared_writer(&mut self) -> &mut dyn Writer;
}

pub unsafe trait Writer {
    fn word_offset(&self) -> usize;
    fn take(&mut self, len: usize) -> Result<&mut [MaybeUninit<usize>], UnexpectedEndError>;
}

#[derive(Clone, Copy, Debug)]
pub struct UnexpectedEndError;

#[cfg(feature = "std")]
unsafe impl AsClearedWriter for Vec<MaybeUninit<usize>> {
    #[inline(always)]
    fn as_cleared_writer(&mut self) -> &mut dyn Writer {
        self.clear();
        self
    }
}

#[cfg(feature = "std")]
unsafe impl<'tape> Writer for Vec<MaybeUninit<usize>> {
    #[inline(always)]
    fn word_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn take(&mut self, words: usize) -> Result<&mut [MaybeUninit<usize>], UnexpectedEndError> {
        let len = self.len();
        self.reserve(words);
        unsafe {
            let slice = core::slice::from_raw_parts_mut(self.as_mut_ptr().add(len), words);
            self.set_len(len + words);
            Ok(slice)
        }
    }
}
