use std::fmt::Debug;

pub(crate) trait ResultExt<T, E> {
    /// Calls unwrap if debug_assertions is enabled and unwrap_unchecked otherwice.
    unsafe fn unwrap_unchecked_on_release(self) -> T;
}

#[cfg(debug_assertions)]
impl<T, E: Debug> ResultExt<T, E> for Result<T, E> {
    #[track_caller]
    #[inline(always)]
    unsafe fn unwrap_unchecked_on_release(self) -> T {
        self.unwrap()
    }
}

#[cfg(not(debug_assertions))]
impl<T, E: Debug> ResultExt<T, E> for Result<T, E> {
    #[inline(always)]
    unsafe fn unwrap_unchecked_on_release(self) -> T {
        self.unwrap_unchecked()
    }
}
