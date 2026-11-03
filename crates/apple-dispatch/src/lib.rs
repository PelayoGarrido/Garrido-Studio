#![forbid(unsafe_code)]

#[cfg(target_vendor = "apple")]
pub struct Queue(&'static dispatch2::DispatchQueue);

#[cfg(target_vendor = "apple")]
impl Queue {
    pub fn main() -> Self {
        Self(dispatch2::DispatchQueue::main())
    }
    pub fn exec_sync<F, R>(&self, work: F) -> R
    where
        F: Send + FnOnce() -> R,
        R: Send,
    {
        let mut result = None;
        self.0.exec_sync(|| result = Some(work()));
        result.expect("GCD completa la llamada síncrona antes de devolver")
    }
}
