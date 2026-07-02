//! Runtime abstraction traits

use std::time::Duration;

#[cfg(feature = "tokio")]
mod tokio {
    use std::time::Duration;

    use futures_util::task::{FutureObj, Spawn, SpawnError};
    #[cfg(feature = "no_send")]
    use futures_util::task::{LocalFutureObj, LocalSpawn};
    use tokio::runtime::Handle;

    use crate::{runtime::Timer, sendable::FutureMaybeSendExt};

    /// A Tokio-backed implementation of [`Spawn`]
    ///
    /// We use this abstraction across the crate for spawning tasks onto the
    /// runtime.
    ///
    /// With `no_send`, the local-spawn implementation delegates to
    /// [`tokio::task::spawn_local`], so it must be used from within a Tokio
    /// [`LocalSet`](tokio::task::LocalSet).
    pub struct TokioSpawner {
        handle: Handle,
    }

    impl TokioSpawner {
        /// Construct a spawner that obtains a handle of the current runtime
        ///
        /// # Panics
        ///
        /// Panics when used outside of the context of a Tokio runtime
        pub fn current() -> Self {
            Self::with_handle(Handle::current())
        }

        /// Construct a spawner with a handle of a specific runtime
        pub fn with_handle(handle: Handle) -> Self {
            Self { handle }
        }
    }

    impl Spawn for TokioSpawner {
        fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
            self.handle.spawn(future);
            Ok(())
        }
    }

    #[cfg(feature = "no_send")]
    impl LocalSpawn for TokioSpawner {
        fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError> {
            let _guard = self.handle.enter();
            tokio::task::spawn_local(future);
            Ok(())
        }
    }

    /// A Tokio-backed implementation of the [`Timer`] trait
    #[derive(Default)]
    pub struct TokioTimer {
        _priv: (),
    }

    impl Timer for TokioTimer {
        fn delay(&self, duration: Duration) -> crate::sendable::MaybeBoxFuture<'static, ()> {
            tokio::time::sleep(duration).boxed_maybe_send()
        }
    }
}
#[cfg(feature = "tokio")]
pub use self::tokio::{TokioSpawner, TokioTimer};
use crate::{MaybeSend, MaybeSync, sendable::MaybeBoxFuture};

/// Timing facilities required by parts of the crate
///
/// The purpose is to make async-graphql integrate nicely with whatever
/// environment you're in.
///
/// Be it Tokio, smol, or even the browser.
pub trait Timer: MaybeSend + MaybeSync + 'static {
    /// Returns a future that resolves after the specified duration
    fn delay(&self, duration: Duration) -> MaybeBoxFuture<'static, ()>;
}

const _: Option<&dyn Timer> = None;
