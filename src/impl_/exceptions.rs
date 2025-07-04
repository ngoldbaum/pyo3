use once_cell::sync::OnceCell;

use crate::{
    sync::OnceCellExt,
    types::{PyAnyMethods, PyType},
    Bound, Py, PyErr, Python,
};

pub struct ImportedExceptionTypeObject {
    imported_value: OnceCell<Py<PyType>>,
    module: &'static str,
    name: &'static str,
}

impl ImportedExceptionTypeObject {
    pub const fn new(module: &'static str, name: &'static str) -> Self {
        Self {
            imported_value: OnceCell::new(),
            module,
            name,
        }
    }

    pub fn get<'py>(&self, py: Python<'py>) -> &Bound<'py, PyType> {
        self.imported_value
            .get_or_try_init_py_attached(py, || {
                let type_object = py
                    .import(self.module)?
                    .getattr(self.name)?
                    .downcast_into()?;
                Ok(type_object.unbind())
            })
            .map(|ty| ty.bind(py))
            .unwrap_or_else(|e: PyErr| {
                panic!(
                    "failed to import exception {}.{}: {}",
                    self.module, self.name, e
                )
            })
    }
}
