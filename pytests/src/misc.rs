use std::cell::RefCell;

use pyo3::{
    prelude::*,
    types::{PyDict, PyString},
};

#[pyfunction]
fn issue_219() {
    // issue 219: attaching inside #[pyfunction] deadlocks.
    Python::attach(|_| {});
}

#[pyclass]
struct LockHolder {
    #[expect(unused, reason = "used to block until sender is dropped")]
    sender: std::sync::mpsc::Sender<()>,
}

#[pyclass]
struct FinalizationLockHolder {
    sender: Option<std::sync::mpsc::Sender<()>>,
    done: SyncReceiver<()>,
    wait_for_thread: bool,
}

impl Drop for FinalizationLockHolder {
    fn drop(&mut self) {
        self.sender.take();
        if self.wait_for_thread {
            self.done
                .recv_timeout(std::time::Duration::from_secs(10))
                .ok();
        }
    }
}

// This will repeatedly attach and detach from the Python interpreter
// once the LockHolder is dropped.
#[pyfunction]
fn hammer_attaching_in_thread() -> LockHolder {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        receiver.recv().ok();
        // now the interpreter has shut down, so hammer the attach API. In buggy
        // versions of PyO3 this will cause a crash.
        loop {
            Python::try_attach(|_py| ());
        }
    });
    LockHolder { sender }
}

/// Wrapper to mark Receiver as Sync.
struct SyncReceiver<T>(std::sync::mpsc::Receiver<T>);

impl<T> std::ops::Deref for SyncReceiver<T> {
    type Target = std::sync::mpsc::Receiver<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// SAFETY: each wrapped receiver is accessed from only one thread.
unsafe impl<T> Sync for SyncReceiver<T> {}

struct FinalizationThreadLocal {
    object: Option<Py<PyAny>>,
    done: std::sync::mpsc::Sender<()>,
}

#[pyclass]
struct MustDropWhileAttached;

impl Drop for MustDropWhileAttached {
    fn drop(&mut self) {
        #[cfg(not(Py_LIMITED_API))]
        {
            // SAFETY: PyGILState_Check can always be called.
            if unsafe { pyo3::ffi::PyGILState_Check() } == 0 {
                std::process::abort();
            }
        }
    }
}

impl Drop for FinalizationThreadLocal {
    fn drop(&mut self) {
        self.object.take();
        self.done.send(()).ok();
    }
}

thread_local! {
    static DETACH_DURING_FINALIZATION_CONTEXT: RefCell<Option<FinalizationThreadLocal>> = const { RefCell::new(None) };
}

#[pyfunction]
fn detach_during_finalization(py: Python<'_>) -> FinalizationLockHolder {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let (done_sender, done_receiver) = std::sync::mpsc::channel();
    let receiver = SyncReceiver(receiver);
    std::thread::spawn(move || {
        Python::attach(|py| {
            DETACH_DURING_FINALIZATION_CONTEXT.with_borrow_mut(|context| {
                *context = Some(FinalizationThreadLocal {
                    object: Some(Py::new(py, MustDropWhileAttached).unwrap().into_any()),
                    done: done_sender,
                });
            });
            ready_sender.send(()).unwrap();
            py.detach(|| {
                receiver.recv().ok();
                // Interpreter is finalizing while we try to reattach after returning
            });
        });
    });
    py.detach(move || ready_receiver.recv()).unwrap();
    FinalizationLockHolder {
        sender: Some(sender),
        done: SyncReceiver(done_receiver),
        // Older CPython releases run TLS destructors here on macOS and musl.
        wait_for_thread: cfg!(any(target_os = "macos", target_env = "musl"))
            && py.version_info() < (3, 13, 8),
    }
}

#[pyfunction]
fn get_type_fully_qualified_name<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyString>> {
    obj.get_type().fully_qualified_name()
}

#[pyfunction]
fn accepts_bool(val: bool) -> bool {
    val
}

#[pyfunction]
fn get_item_and_run_callback(dict: Bound<'_, PyDict>, callback: Bound<'_, PyAny>) -> PyResult<()> {
    // This function gives the opportunity to run a pure-Python callback so that
    // gevent can instigate a context switch. This had problematic interactions
    // with PyO3's removed "GIL Pool".
    // For context, see https://github.com/PyO3/pyo3/issues/3668
    let item = dict.get_item("key")?.expect("key not found in dict");
    let string = item.to_string();
    callback.call0()?;
    assert_eq!(item.to_string(), string);
    Ok(())
}

#[pymodule]
pub mod misc {
    #[pymodule_export]
    use super::{
        accepts_bool, detach_during_finalization, get_item_and_run_callback,
        get_type_fully_qualified_name, hammer_attaching_in_thread, issue_219,
    };
}
