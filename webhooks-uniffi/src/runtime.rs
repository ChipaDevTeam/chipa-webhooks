use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("failed to create tokio runtime"))
}

pub(crate) fn block_on<F: Future>(f: F) -> F::Output {
    runtime().block_on(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_on_trivial_async() {
        let result = block_on(async { 42u32 });
        assert_eq!(result, 42);
    }

    #[test]
    fn block_on_twice_same_runtime() {
        let a = block_on(async { 1u32 });
        let b = block_on(async { 2u32 });
        assert_eq!(a + b, 3);
    }
}
