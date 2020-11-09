//! Tapes to which programs are written.
//!
//! `Vec<MaybeUninit<usize>>` implements both `AsClearedWriter` and `Writer`
//! when the `std` feature is enabled.

use core::mem::MaybeUninit;

/// Types from which a cleared writer can be obtained.
///
/// # Safety
///
/// Types implementing this trait represent a tape, which for safety reasons
/// must respect various invariants that I'm too lazy to list right now,
/// but more or less it just represents a glorified slice that can be
/// made longer.
pub unsafe trait AsClearedWriter: AsRef<[MaybeUninit<usize>]> {
    /// Returns a cleared writer from this value.
    fn as_cleared_writer(&mut self) -> &mut dyn Writer;
}

/// Types that can be written into.
///
/// # Safety
///
/// This trait is unsafe for the same reasons as `AsClearedWriter`.
pub unsafe trait Writer {
    /// Returns the current position of the writer, in words.
    fn word_offset(&self) -> usize;

    /// Take `n` words from the writer, starting at the current position.
    fn take(&mut self, n: usize) -> Result<&mut [MaybeUninit<usize>], UnexpectedEndError>;
}

/// An error that signals that the end of the tape was unexpectedly reached.
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
