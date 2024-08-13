use crate::{PyMutex, PyObject};

#[repr(C)]
#[cfg(Py_GIL_DISABLED)]
pub struct PyCriticalSection {
    _cs_prev: usize,
    _cs_mutex: *mut PyMutex,
}

#[repr(C)]
#[cfg(Py_GIL_DISABLED)]
pub struct PyCriticalSection2 {
    _cs_base: PyCriticalSection,
    _cs_mutex2: *mut PyMutex,
}

#[cfg(not(Py_GIL_DISABLED))]
opaque_struct!(PyCriticalSection);

#[cfg(not(Py_GIL_DISABLED))]
opaque_struct!(PyCriticalSection2);

extern "C" {
    pub fn PyCriticalSection_Begin(c: *mut PyCriticalSection, op: *mut PyObject);
    pub fn PyCriticalSection_End(c: *mut PyCriticalSection);
    pub fn PyCriticalSection2_Begin(c: *mut PyCriticalSection2, a: *mut PyObject, b: *mut PyObject);
    pub fn PyCriticalSection2_End(c: *mut PyCriticalSection2);
}

pub unsafe fn Py_BEGIN_CRITICAL_SECTION(op: *mut PyObject) -> Option<PyCriticalSection> {
    #[cfg(Py_GIL_DISABLED)]
    {
        let mut py_cs: PyCriticalSection = unsafe { std::mem::zeroed() };
        unsafe { PyCriticalSection_Begin(&mut py_cs, op) };
        Some(py_cs)
    }
    #[cfg(not(Py_GIL_DISABLED))]
    None
}

pub fn Py_END_CRITICAL_SECTION(cs: Option<PyCriticalSection>) {
    #[cfg(Py_GIL_DISABLED)]
    if let Some(mut cs) = cs {
        unsafe { PyCriticalSection_End(&mut cs) };
    }
}
