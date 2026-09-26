//! Shared stream transport for native renderers in all frontends. Each renderer
//! writes its existing PCM protocol to a private socket/pipe on a worker thread.
//! Readers provide backpressure and cancellation without a child executable.

use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::io::{self, Read};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

thread_local! {
    static CANCELLED: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

/// Called by native renderers while seeking or waiting for the emulated
/// driver to produce audio. A closed pipe alone cannot cancel those phases.
#[unsafe(no_mangle)]
pub extern "C" fn kog_decoder_cancelled() -> bool {
    CANCELLED.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    })
}

#[cfg(windows)]
use std::fs::File;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::fd::IntoRawFd;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, IntoRawHandle};

pub struct EmbeddedHelper {
    reader: Option<Box<dyn Read + Send>>,
    worker: Option<JoinHandle<Result<(), String>>>,
    cancelled: Arc<AtomicBool>,
    #[cfg(unix)]
    socket: Option<UnixStream>,
}

impl EmbeddedHelper {
    pub fn spawn(
        job: impl FnOnce(isize, &mut [c_char]) -> i32 + Send + 'static,
    ) -> Result<Self, String> {
        #[cfg(unix)]
        let (reader, writer) = UnixStream::pair().map_err(|error| error.to_string())?;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let enabled: libc::c_int = 1;
            let status = unsafe {
                libc::setsockopt(
                    writer.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_NOSIGPIPE,
                    (&raw const enabled).cast(),
                    std::mem::size_of_val(&enabled) as libc::socklen_t,
                )
            };
            if status != 0 {
                return Err(format!(
                    "configuring PCM stream: {}",
                    io::Error::last_os_error()
                ));
            }
        }
        #[cfg(windows)]
        let (reader, writer) = {
            use windows_sys::Win32::System::Pipes::CreatePipe;
            let (mut read, mut write) = (std::ptr::null_mut(), std::ptr::null_mut());
            let status = unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 0) };
            if status == 0 {
                return Err(format!("creating PCM pipe: {}", io::Error::last_os_error()));
            }
            (unsafe { File::from_raw_handle(read) }, unsafe {
                File::from_raw_handle(write)
            })
        };
        #[cfg(unix)]
        let socket = Some(reader.try_clone().map_err(|error| error.to_string())?);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let worker = thread::spawn(move || {
            CANCELLED.with(|state| *state.borrow_mut() = Some(worker_cancelled));
            #[cfg(any(target_os = "linux", target_os = "android"))]
            unsafe {
                // A cancelled read closes the peer. Keep EPIPE local to this
                // worker instead of allowing SIGPIPE to terminate the app.
                let mut mask = std::mem::zeroed::<libc::sigset_t>();
                libc::sigemptyset(&mut mask);
                libc::sigaddset(&mut mask, libc::SIGPIPE);
                libc::pthread_sigmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut());
            }
            let mut message = [0 as c_char; 1024];
            // The C adapter owns and closes the raw descriptor/handle.
            #[cfg(unix)]
            let descriptor = writer.into_raw_fd() as isize;
            #[cfg(windows)]
            let descriptor = writer.into_raw_handle() as isize;
            let result = job(descriptor, &mut message);
            if result == 0 {
                Ok(())
            } else {
                let detail = unsafe { CStr::from_ptr(message.as_ptr()) }.to_string_lossy();
                Err(if detail.is_empty() {
                    format!("native renderer returned {result}")
                } else {
                    detail.into_owned()
                })
            }
        });
        Ok(Self {
            reader: Some(Box::new(reader)),
            worker: Some(worker),
            cancelled,
            #[cfg(unix)]
            socket,
        })
    }

    pub fn failure(&mut self) -> Option<String> {
        self.close_reader();
        match self.worker.take()?.join() {
            Ok(Ok(())) => None,
            Ok(Err(message)) => Some(message),
            Err(_) => Some("native renderer panicked".to_owned()),
        }
    }

    fn close_reader(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        #[cfg(unix)]
        if let Some(socket) = self.socket.take() {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
        self.reader.take();
    }

    pub fn cancel(&mut self) {
        self.close_reader();
        // The next write ends the worker; avoid blocking the UI thread.
        self.worker.take();
    }
}

impl Read for EmbeddedHelper {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reader
            .as_mut()
            .map_or(Ok(0), |reader| reader.read(output))
    }
}

impl Drop for EmbeddedHelper {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn cancellation_reaches_native_worker_before_pcm_is_written() {
        let (started, wait_started) = mpsc::channel();
        let (finished, wait_finished) = mpsc::channel();
        let mut stream = EmbeddedHelper::spawn(move |descriptor, _error| {
            started.send(()).unwrap();
            while !kog_decoder_cancelled() {
                thread::park_timeout(Duration::from_millis(1));
            }
            unsafe { libc::close(descriptor as libc::c_int) };
            finished.send(()).unwrap();
            0
        })
        .unwrap();
        wait_started.recv_timeout(Duration::from_secs(2)).unwrap();
        stream.cancel();
        wait_finished.recv_timeout(Duration::from_secs(2)).unwrap();
        // A cancellation flag belongs to its renderer thread, not the caller.
        assert!(!kog_decoder_cancelled());
    }
}
