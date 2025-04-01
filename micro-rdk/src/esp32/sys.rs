use std::sync::OnceLock;

use async_lock::Mutex as AsyncMutex;

pub fn get_system_lock() -> &'static AsyncMutex<()> {
    static SYSTEM_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    SYSTEM_LOCK.get_or_init(|| AsyncMutex::new(()))
}
