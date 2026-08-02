#[cfg(unix)]
mod unix;
#[cfg(not(unix))]
mod unsupported;

#[cfg(unix)]
pub(crate) use unix::EventStream;
#[cfg(not(unix))]
pub(crate) use unsupported::EventStream;
