use once_cell::sync::OnceCell;
use tokio::runtime::{Builder, Runtime};

static TOKIO_RT: OnceCell<Runtime> = OnceCell::new();

/// Initializes the shared tokio runtime. Call once at the top of `main()`;
/// the thread name shows up in profilers/debuggers per binary.
pub fn init(thread_name: &str) {
    let rt = Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .thread_name(thread_name)
        .build()
        .expect("tokio runtime");
    TOKIO_RT.set(rt).expect("runtime already initialized");
}

/// Returns the shared runtime. Panics if [`init`] has not been called.
pub fn rt() -> &'static Runtime {
    TOKIO_RT.get().expect("call rt::init() at the top of main()")
}
