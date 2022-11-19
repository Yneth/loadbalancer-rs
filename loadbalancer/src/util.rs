use std::future::Future;

use anyhow::Result;

// slightly improves performance over elvis operator
// to be used in performance critical code
#[macro_export]
macro_rules! ok {
    ($result:expr) => {
        match $result {
            Err(e) => return Err(anyhow::Error::from(e)),
            Ok(res) => res,
        }
    };
}

fn set_max_open_files() -> std::io::Result<u64> {
    #[cfg(unix)]
    {
        rlimit::increase_nofile_limit(u64::MAX).map_err(Into::into)
    }

    #[cfg(windows)]
    {
        const WINDOWS_MAX_FILES: u32 = 8192;
        rlimit::setmaxstdio(WINDOWS_MAX_FILES).map(u64::from).map_err(Into::into)
    }
}

pub fn setup_limits() {
    match set_max_open_files() {
        Ok(lim) => tracing::info!("maxfiles was successfully set: {}", lim),
        Err(e) => tracing::warn!("maxfiles failed to set, reason: {:?}", e),
    }
}

pub async fn retry_immediate<F, Fut, R>(retries: usize, f: F) -> Result<R>
    where
        Fut: Future<Output=Result<R>>,
        F: FnOnce(usize) -> Fut + Copy
{
    let mut result = f(0).await;
    for i in 0..retries {
        if result.is_err() {
            result = f(i + 1).await;
        } else {
            break;
        }
    }
    result
}
