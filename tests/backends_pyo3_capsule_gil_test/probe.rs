#[cfg(test)]
mod capsule_runtime_probe {
    use super::*;

    #[pyfunction]
    fn core_active() -> bool {
        sample_core::core_active()
    }

    #[test]
    fn python_threads_progress_during_core_calls() {
        Python::initialize();
        Python::attach(|py| {
            let module = PyModule::new(py, "_sample").unwrap();
            _sample(&module).unwrap();
            module
                .add_function(wrap_pyfunction!(core_active, &module).unwrap())
                .unwrap();
            let registry = Py::new(
                py,
                Registry {
                    inner: std::sync::Arc::new(sample_core::Registry),
                },
            )
            .unwrap();
            module.add("registry", registry).unwrap();
            let globals = pyo3::types::PyDict::new(py);
            globals.set_item("native", module).unwrap();
            globals.set_item("expected_release", EXPECT_RELEASE).unwrap();
            let script = std::ffi::CString::new(include_str!("probe.py")).unwrap();
            py.run(&script, Some(&globals), None).unwrap();
        });
    }
}
