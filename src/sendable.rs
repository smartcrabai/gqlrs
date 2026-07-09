//! Conditional `Send`/`Sync` bounds for single-threaded runtime support.
//!
//! When the `no_send` feature is enabled, [`MaybeSend`] and [`MaybeSync`] are
//! blanket-implemented for all types, effectively removing the `Send + Sync`
//! requirements from the crate's traits. This allows the crate to be used in
//! single-threaded runtimes such as Cloudflare Workers (`workers-rs`) that
//! cannot satisfy `Send + Sync` bounds.
//!
//! When the `no_send` feature is **not** enabled (the default),
//! `MaybeSend` requires `Send` and `MaybeSync` requires `Sync`.

#[cfg(not(feature = "no_send"))]
mod inner {
    /// A conditional `Send` bound.
    ///
    /// Equivalent to `Send` when `no_send` is not enabled.
    /// Blanket-implemented for all `Send` types.
    pub trait MaybeSend: Send {}
    impl<T: Send + ?Sized> MaybeSend for T {}

    /// A conditional `Sync` bound.
    ///
    /// Equivalent to `Sync` when `no_send` is not enabled.
    /// Blanket-implemented for all `Sync` types.
    pub trait MaybeSync: Sync {}
    impl<T: Sync + ?Sized> MaybeSync for T {}
}

#[cfg(feature = "no_send")]
mod inner {
    /// A conditional `Send` bound.
    ///
    /// Equivalent to `Send` when `no_send` is not enabled.
    /// Blanket-implemented for all types when `no_send` is enabled.
    pub trait MaybeSend {}
    impl<T: ?Sized> MaybeSend for T {}

    /// A conditional `Sync` bound.
    ///
    /// Equivalent to `Sync` when `no_send` is not enabled.
    /// Blanket-implemented for all types when `no_send` is enabled.
    pub trait MaybeSync {}
    impl<T: ?Sized> MaybeSync for T {}
}

use std::{future::Future, pin::Pin};

use futures_util::Stream;
pub use inner::*;

#[doc(hidden)]
#[cfg(not(feature = "no_send"))]
pub type MaybeBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[doc(hidden)]
#[cfg(feature = "no_send")]
pub type MaybeBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[doc(hidden)]
#[cfg(not(feature = "no_send"))]
pub type MaybeBoxStream<'a, T> = Pin<Box<dyn Stream<Item = T> + Send + 'a>>;

#[doc(hidden)]
#[cfg(feature = "no_send")]
pub type MaybeBoxStream<'a, T> = Pin<Box<dyn Stream<Item = T> + 'a>>;

pub(crate) trait FutureMaybeSendExt: Future + Sized {
    fn boxed_maybe_send<'a>(self) -> MaybeBoxFuture<'a, Self::Output>
    where
        Self: MaybeSend + 'a,
    {
        Box::pin(self)
    }
}

impl<T: Future + Sized> FutureMaybeSendExt for T {}

#[allow(dead_code)]
pub(crate) trait StreamMaybeSendExt: Stream + Sized {
    fn boxed_maybe_send<'a>(self) -> MaybeBoxStream<'a, Self::Item>
    where
        Self: MaybeSend + 'a,
    {
        Box::pin(self)
    }
}

impl<T: Stream + Sized> StreamMaybeSendExt for T {}
