use std::sync::atomic::{AtomicBool, Ordering};

pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);

pub fn register_shutdown_handler() {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = shutdown_handler as *const () as usize;
        sa.sa_flags = libc::SA_RESTART;
        libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
    }
}

extern "C" fn shutdown_handler(_sig: libc::c_int) {
    SHUTDOWN.store(true, Ordering::Release);
}

pub fn block_signals() -> anyhow::Result<()> {
    use nix::sys::signal::{SigSet, SigmaskHow, Signal, sigprocmask};
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGTERM);
    mask.add(Signal::SIGINT);
    sigprocmask(SigmaskHow::SIG_BLOCK, Some(&mask), None)?;
    Ok(())
}

pub fn create_signal_fd() -> anyhow::Result<i32> {
    let mut mask: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, libc::SIGTERM);
        libc::sigaddset(&mut mask, libc::SIGINT);
    }
    let fd = unsafe { libc::signalfd(-1, &mask, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK) };
    if fd < 0 {
        anyhow::bail!("signalfd failed: {}", std::io::Error::last_os_error());
    }
    Ok(fd)
}
