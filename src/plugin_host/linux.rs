use super::{Error, Limit, Result, getrlimit, setrlimit};

unsafe extern "C" {
    fn prctl(option: i32, ...) -> i32;
}
pub(super) fn jail() -> Result<()> {
    // SAFETY: solo se reducen límites del proceso worker antes de cargar el módulo.
    unsafe {
        for (resource, maximum) in [(0, 10), (9, 384 * 1024 * 1024), (1, 0), (4, 0), (7, 64)] {
            let mut existing = Limit { current: 0, max: 0 };
            if getrlimit(resource, &mut existing) != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let maximum = maximum.min(existing.max);
            if setrlimit(
                resource,
                &Limit {
                    current: maximum,
                    max: maximum,
                },
            ) != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        if prctl(38, 1usize, 0usize, 0usize, 0usize) != 0 {
            return Err(Error::Unsupported("No se pudo fijar no_new_privs"));
        }
    }
    Ok(())
}
