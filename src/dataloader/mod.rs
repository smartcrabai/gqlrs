//! Batch loading support, used to solve N+1 problem.
//!
//! # Examples
//!
//! ```rust
//! use gqlrs::*;
//! use gqlrs::dataloader::*;
//! use std::collections::{HashSet, HashMap};
//! use std::convert::Infallible;
//! use gqlrs::dataloader::Loader;
//! use gqlrs::runtime::{TokioSpawner, TokioTimer};
//!
//! /// This loader simply converts the integer key into a string value.
//! struct MyLoader;
//!
//! #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
//! impl Loader<i32, String> for MyLoader {
//!     type Error = Infallible;
//!
//!     async fn load(&self, keys: &[i32]) -> Result<HashMap<i32, String>, Self::Error> {
//!         // Use `MyLoader` to load data.
//!         Ok(keys.iter().copied().map(|n| (n, n.to_string())).collect())
//!     }
//! }
//!
//! struct Query;
//!
//! #[Object]
//! impl Query {
//!     async fn value(&self, ctx: &Context<'_>, n: i32) -> Option<String> {
//!         ctx.data::<DataLoader<MyLoader>>().unwrap().load_one(n).await.unwrap()
//!     }
//! }
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async move {
//! let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
//! let query = r#"
//!     {
//!         v1: value(n: 1)
//!         v2: value(n: 2)
//!         v3: value(n: 3)
//!         v4: value(n: 4)
//!         v5: value(n: 5)
//!     }
//! "#;
//! let request = Request::new(query).data(DataLoader::new(MyLoader, TokioSpawner::current(), TokioTimer::default()));
//! let res = schema.execute(request).await.into_result().unwrap().data;
//!
//! assert_eq!(res, value!({
//!     "v1": "1",
//!     "v2": "2",
//!     "v3": "3",
//!     "v4": "4",
//!     "v5": "5",
//! }));
//! # });
//! ```

mod cache;

#[cfg(not(feature = "boxed-trait"))]
use std::future::Future;
use std::{
    any::{Any, TypeId},
    borrow::Cow,
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub use cache::{CacheFactory, CacheStorage, HashMapCache, LruCache, NoCache};
use futures_channel::{mpsc, oneshot};
#[cfg(not(feature = "boxed-trait"))]
use futures_util::stream::Stream;
use futures_util::{
    stream::{self, BoxStream, StreamExt, TryStreamExt},
    task::{Spawn, SpawnExt},
};
use rustc_hash::FxBuildHasher;
#[cfg(feature = "tracing")]
use tracing::{Instrument, info_span, instrument};

use crate::runtime::Timer;

type FxHashMap<K, V> = scc::HashMap<K, V, FxBuildHasher>;

#[allow(clippy::type_complexity)]
struct ResSender<K, V, T>
where
    K: Send + Sync + Hash + Eq + Clone + 'static,
    V: Send + Sync + Clone + 'static,
    T: Loader<K, V>,
{
    use_cache_values: HashMap<K, V>,
    tx: oneshot::Sender<Result<HashMap<K, V>, <T as Loader<K, V>>::Error>>,
}

struct StreamSender<K, V, T>
where
    K: Send + Sync + Hash + Eq + Clone + 'static,
    V: Send + Sync + Clone + 'static,
    T: Loader<K, V>,
{
    tx: mpsc::UnboundedSender<StreamResult<K, V, T::Error>>,
}

struct Requests<K, V, T>
where
    K: Send + Sync + Hash + Eq + Clone + 'static,
    V: Send + Sync + Clone + 'static,
    T: Loader<K, V>,
{
    keys: HashSet<K>,
    pending: Vec<(HashSet<K>, ResSender<K, V, T>)>,
    stream_keys: HashSet<K>,
    stream_pending: Vec<(HashSet<K>, StreamSender<K, V, T>)>,
    cache_storage: Box<dyn CacheStorage<Key = K, Value = V>>,
    disable_cache: bool,
}

type StreamResult<K, V, E> = Result<(K, V), E>;
type LoaderStream<'a, K, V, E> = BoxStream<'a, StreamResult<K, V, E>>;
type KeysAndSender<K, V, T> = (HashSet<K>, Vec<(HashSet<K>, ResSender<K, V, T>)>);
type StreamKeysAndSender<K, V, T> = (HashSet<K>, Vec<(HashSet<K>, StreamSender<K, V, T>)>);

impl<K, V, T> Requests<K, V, T>
where
    K: Send + Sync + Hash + Eq + Clone + 'static,
    V: Send + Sync + Clone + 'static,
    T: Loader<K, V>,
{
    fn new<C: CacheFactory>(cache_factory: &C) -> Self {
        Self {
            keys: Default::default(),
            pending: Vec::new(),
            stream_keys: Default::default(),
            stream_pending: Vec::new(),
            cache_storage: cache_factory.create::<K, V>(),
            disable_cache: false,
        }
    }

    fn take(&mut self) -> KeysAndSender<K, V, T> {
        (
            std::mem::take(&mut self.keys),
            std::mem::take(&mut self.pending),
        )
    }

    fn take_stream(&mut self) -> StreamKeysAndSender<K, V, T> {
        (
            std::mem::take(&mut self.stream_keys),
            std::mem::take(&mut self.stream_pending),
        )
    }
}

/// Trait for batch loading.
///
/// Loaders are distinguished by both the key type `K` and the returned value
/// type `V`. This allows multiple loaders on the same type to use the same key
/// type but different return types without colliding in the internal request
/// map.
#[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
pub trait Loader<K, V>: Send + Sync + 'static
where
    K: Send + Sync + Hash + Eq + Clone + 'static,
    V: Send + Sync + Clone + 'static,
{
    /// Type of error.
    type Error: Send + Clone + 'static;

    /// Load the data set specified by the `keys`.
    #[cfg(feature = "boxed-trait")]
    async fn load(&self, keys: &[K]) -> Result<HashMap<K, V>, Self::Error>;

    /// Load the data set specified by the `keys`.
    #[cfg(not(feature = "boxed-trait"))]
    fn load(&self, keys: &[K]) -> impl Future<Output = Result<HashMap<K, V>, Self::Error>> + Send;

    /// Load the data set specified by the `keys` as a stream.
    ///
    /// Returns a stream that yields key-value pairs as they become available.
    /// This is useful for loaders that can return results progressively rather
    /// than waiting for the entire batch to complete.
    ///
    /// The default implementation wraps [`load`](Loader::load) in a stream.
    #[cfg(feature = "boxed-trait")]
    fn load_stream<'a>(&'a self, keys: &'a [K]) -> LoaderStream<'a, K, V, Self::Error> {
        Box::pin(
            stream::once(async move {
                self.load(keys)
                    .await
                    .map(|map| stream::iter(map.into_iter().map(Ok)))
            })
            .try_flatten(),
        )
    }

    /// Load the data set specified by the `keys` as a stream.
    ///
    /// Returns a stream that yields key-value pairs as they become available.
    /// This is useful for loaders that can return results progressively rather
    /// than waiting for the entire batch to complete.
    ///
    /// The default implementation wraps [`load`](Loader::load) in a stream.
    #[cfg(not(feature = "boxed-trait"))]
    fn load_stream<'a>(
        &'a self,
        keys: &'a [K],
    ) -> impl Stream<Item = Result<(K, V), Self::Error>> + Send + 'a {
        stream::once(async move {
            self.load(keys)
                .await
                .map(|map| stream::iter(map.into_iter().map(Ok)))
        })
        .try_flatten()
    }
}

struct DataLoaderInner<T> {
    requests: FxHashMap<TypeId, Box<dyn Any + Sync + Send>>,
    loader: T,
}

impl<T> DataLoaderInner<T> {
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    async fn do_load<K, V>(&self, disable_cache: bool, (keys, senders): KeysAndSender<K, V, T>)
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let tid = TypeId::of::<(K, V)>();
        let keys = keys.into_iter().collect::<Vec<_>>();

        match self.loader.load(&keys).await {
            Ok(values) => {
                // update cache
                let mut entry = self.requests.get_async(&tid).await.unwrap();

                let typed_requests = entry.get_mut().downcast_mut::<Requests<K, V, T>>().unwrap();

                let disable_cache = typed_requests.disable_cache || disable_cache;
                if !disable_cache {
                    for (key, value) in &values {
                        typed_requests
                            .cache_storage
                            .insert(Cow::Borrowed(key), Cow::Borrowed(value));
                    }
                }

                // send response
                for (keys, sender) in senders {
                    let mut res = HashMap::new();
                    res.extend(sender.use_cache_values);
                    for key in &keys {
                        res.extend(values.get(key).map(|value| (key.clone(), value.clone())));
                    }
                    sender.tx.send(Ok(res)).ok();
                }
            }
            Err(err) => {
                for (_, sender) in senders {
                    sender.tx.send(Err(err.clone())).ok();
                }
            }
        }
    }

    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    async fn do_load_stream<K, V>(
        &self,
        disable_cache: bool,
        (keys, senders): StreamKeysAndSender<K, V, T>,
    ) where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let tid = TypeId::of::<K>();
        let keys_vec = keys.iter().cloned().collect::<Vec<_>>();
        let mut stream = Box::pin(self.loader.load_stream(&keys_vec));

        while let Some(item) = stream.next().await {
            match item {
                Ok((key, value)) => {
                    if !disable_cache {
                        let mut entry = self.requests.get_async(&tid).await.unwrap();
                        let typed_requests =
                            entry.get_mut().downcast_mut::<Requests<K, V, T>>().unwrap();
                        if !typed_requests.disable_cache {
                            typed_requests
                                .cache_storage
                                .insert(Cow::Borrowed(&key), Cow::Borrowed(&value));
                        }
                    }

                    for (req_keys, sender) in &senders {
                        if req_keys.contains(&key) {
                            sender
                                .tx
                                .unbounded_send(Ok((key.clone(), value.clone())))
                                .ok();
                        }
                    }
                }
                Err(err) => {
                    for (_, sender) in &senders {
                        sender.tx.unbounded_send(Err(err.clone())).ok();
                    }
                    break;
                }
            }
        }
    }
}

/// Data loader.
///
/// Reference: <https://github.com/facebook/dataloader>
pub struct DataLoader<T, C = NoCache> {
    inner: Arc<DataLoaderInner<T>>,
    cache_factory: C,
    delay: Duration,
    max_batch_size: usize,
    disable_cache: AtomicBool,
    spawner: Box<dyn Spawn + Send + Sync>,
    timer: Arc<dyn Timer>,
}

impl<T> DataLoader<T, NoCache> {
    /// Use `Loader` to create a [DataLoader] that does not cache records.
    pub fn new<S, TR>(loader: T, spawner: S, timer: TR) -> Self
    where
        S: Spawn + Send + Sync + 'static,
        TR: Timer,
    {
        Self {
            inner: Arc::new(DataLoaderInner {
                requests: Default::default(),
                loader,
            }),
            cache_factory: NoCache,
            delay: Duration::ZERO,
            max_batch_size: 1000,
            disable_cache: false.into(),
            spawner: Box::new(spawner),
            timer: Arc::new(timer),
        }
    }
}

impl<T, C: CacheFactory> DataLoader<T, C> {
    /// Use `Loader` to create a [DataLoader] with a cache factory.
    pub fn with_cache<S, TR>(loader: T, spawner: S, timer: TR, cache_factory: C) -> Self
    where
        S: Spawn + Send + Sync + 'static,
        TR: Timer,
    {
        Self {
            inner: Arc::new(DataLoaderInner {
                requests: Default::default(),
                loader,
            }),
            cache_factory,
            delay: Duration::ZERO,
            max_batch_size: 1000,
            disable_cache: false.into(),
            spawner: Box::new(spawner),
            timer: Arc::new(timer),
        }
    }

    /// Specify the delay time for loading data, the default is `0ms` (no
    /// delay).
    #[must_use]
    pub fn delay(self, delay: Duration) -> Self {
        Self { delay, ..self }
    }

    /// pub fn Specify the max batch size for loading data, the default is
    /// `1000`.
    ///
    /// If the keys waiting to be loaded reach the threshold, they are loaded
    /// immediately.
    #[must_use]
    pub fn max_batch_size(self, max_batch_size: usize) -> Self {
        Self {
            max_batch_size,
            ..self
        }
    }

    /// Get the loader.
    #[inline]
    pub fn loader(&self) -> &T {
        &self.inner.loader
    }

    /// Enable/Disable cache of all loaders.
    pub fn enable_all_cache(&self, enable: bool) {
        self.disable_cache.store(!enable, Ordering::SeqCst);
    }

    /// Enable/Disable cache of specified loader.
    pub async fn enable_cache<K, V>(&self, enable: bool)
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let tid = TypeId::of::<(K, V)>();
        let mut entry = self.inner.requests.get_async(&tid).await.unwrap();
        let typed_requests = entry.get_mut().downcast_mut::<Requests<K, V, T>>().unwrap();
        typed_requests.disable_cache = !enable;
    }

    /// Use this `DataLoader` load a data.
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub async fn load_one<K, V>(&self, key: K) -> Result<Option<V>, <T as Loader<K, V>>::Error>
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let mut values = self.load_many(std::iter::once(key.clone())).await?;
        Ok(values.remove(&key))
    }

    /// Use this `DataLoader` to load some data.
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub async fn load_many<K, V>(
        &self,
        keys: impl IntoIterator<Item = K>,
    ) -> Result<HashMap<K, V>, <T as Loader<K, V>>::Error>
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        enum Action<K, V, T>
        where
            K: Send + Sync + Hash + Eq + Clone + 'static,
            V: Send + Sync + Clone + 'static,
            T: Loader<K, V>,
        {
            ImmediateLoad(KeysAndSender<K, V, T>),
            StartFetch,
            Delay,
        }

        let tid = TypeId::of::<(K, V)>();

        let (action, rx) = {
            let mut entry = self
                .inner
                .requests
                .entry_async(tid)
                .await
                .or_insert_with(|| Box::new(Requests::<K, V, T>::new(&self.cache_factory)));

            let typed_requests = entry.downcast_mut::<Requests<K, V, T>>().unwrap();

            let prev_count = typed_requests.keys.len();
            let mut keys_set = HashSet::new();
            let mut use_cache_values = HashMap::new();

            if typed_requests.disable_cache || self.disable_cache.load(Ordering::SeqCst) {
                keys_set = keys.into_iter().collect();
            } else {
                for key in keys {
                    if let Some(value) = typed_requests.cache_storage.get(&key) {
                        // Already in cache
                        use_cache_values.insert(key.clone(), value);
                    } else {
                        keys_set.insert(key);
                    }
                }
            }

            if !use_cache_values.is_empty() && keys_set.is_empty() {
                return Ok(use_cache_values);
            } else if use_cache_values.is_empty() && keys_set.is_empty() {
                return Ok(Default::default());
            }

            typed_requests.keys.extend(keys_set.clone());
            let (tx, rx) = oneshot::channel();
            typed_requests.pending.push((
                keys_set,
                ResSender {
                    use_cache_values,
                    tx,
                },
            ));

            if typed_requests.keys.len() >= self.max_batch_size {
                (Action::ImmediateLoad(typed_requests.take()), rx)
            } else {
                (
                    if !typed_requests.keys.is_empty() && prev_count == 0 {
                        Action::StartFetch
                    } else {
                        Action::Delay
                    },
                    rx,
                )
            }
        };

        match action {
            Action::ImmediateLoad(keys) => {
                let inner = self.inner.clone();
                let disable_cache = self.disable_cache.load(Ordering::SeqCst);
                let task = async move { inner.do_load(disable_cache, keys).await };
                #[cfg(feature = "tracing")]
                let task = task
                    .instrument(info_span!("immediate_load"))
                    .in_current_span();

                let _ = self.spawner.spawn(task);
            }
            Action::StartFetch => {
                let inner = self.inner.clone();
                let disable_cache = self.disable_cache.load(Ordering::SeqCst);
                let delay = self.delay;
                let timer = self.timer.clone();

                let task = async move {
                    timer.delay(delay).await;

                    let keys = {
                        let mut entry = inner.requests.get_async(&tid).await.unwrap();
                        let typed_requests = entry.downcast_mut::<Requests<K, V, T>>().unwrap();
                        typed_requests.take()
                    };

                    if !keys.0.is_empty() {
                        inner.do_load(disable_cache, keys).await
                    }
                };
                #[cfg(feature = "tracing")]
                let task = task.instrument(info_span!("start_fetch")).in_current_span();
                let _ = self.spawner.spawn(task);
            }
            Action::Delay => {}
        }

        rx.await.unwrap()
    }

    /// Use this `DataLoader` to load some data as a stream.
    ///
    /// This uses the [`Loader::load_stream`] method and yields key-value pairs
    /// as they become available.
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub async fn load_many_stream<K, V, I>(
        &self,
        keys: I,
    ) -> LoaderStream<'static, K, V, T::Error>
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        I: IntoIterator<Item = K>,
        T: Loader<K, V>,
    {
        enum Action<K: Send + Sync + Hash + Eq + Clone + 'static, V: Send + Sync + Clone + 'static, T: Loader<K, V>> {
            ImmediateLoad(StreamKeysAndSender<K, V, T>),
            StartFetch,
            Delay,
        }

        let tid = TypeId::of::<K>();

        let (action, rx) = {
            let mut entry = self
                .inner
                .requests
                .entry_async(tid)
                .await
                .or_insert_with(|| Box::new(Requests::<K, V, T>::new(&self.cache_factory)));

            let typed_requests = entry.downcast_mut::<Requests<K, V, T>>().unwrap();

            let prev_count = typed_requests.stream_keys.len();
            let mut keys_set = HashSet::new();
            let mut use_cache_values = HashMap::new();

            if typed_requests.disable_cache || self.disable_cache.load(Ordering::SeqCst) {
                keys_set = keys.into_iter().collect();
            } else {
                for key in keys {
                    if let Some(value) = typed_requests.cache_storage.get(&key) {
                        use_cache_values.insert(key.clone(), value);
                    } else {
                        keys_set.insert(key);
                    }
                }
            }

            if keys_set.is_empty() {
                return Box::pin(stream::iter(use_cache_values.into_iter().map(Ok)));
            }

            typed_requests.stream_keys.extend(keys_set.clone());
            let (tx, rx) = mpsc::unbounded();
            for value in use_cache_values {
                tx.unbounded_send(Ok(value)).ok();
            }
            typed_requests
                .stream_pending
                .push((keys_set, StreamSender { tx }));

            if typed_requests.stream_keys.len() >= self.max_batch_size {
                (Action::ImmediateLoad(typed_requests.take_stream()), rx)
            } else {
                (
                    if !typed_requests.stream_keys.is_empty() && prev_count == 0 {
                        Action::StartFetch
                    } else {
                        Action::Delay
                    },
                    rx,
                )
            }
        };

        match action {
            Action::ImmediateLoad(keys) => {
                let inner = self.inner.clone();
                let disable_cache = self.disable_cache.load(Ordering::SeqCst);
                let task = async move { inner.do_load_stream(disable_cache, keys).await };
                #[cfg(feature = "tracing")]
                let task = task
                    .instrument(info_span!("immediate_load_stream"))
                    .in_current_span();

                let _ = self.spawner.spawn(task);
            }
            Action::StartFetch => {
                let inner = self.inner.clone();
                let disable_cache = self.disable_cache.load(Ordering::SeqCst);
                let delay = self.delay;
                let timer = self.timer.clone();

                let task = async move {
                    timer.delay(delay).await;

                    let keys = {
                        let mut entry = inner.requests.get_async(&tid).await.unwrap();
                        let typed_requests = entry.downcast_mut::<Requests<K, V, T>>().unwrap();
                        typed_requests.take_stream()
                    };

                    if !keys.0.is_empty() {
                        inner.do_load_stream(disable_cache, keys).await
                    }
                };
                #[cfg(feature = "tracing")]
                let task = task
                    .instrument(info_span!("start_fetch_stream"))
                    .in_current_span();
                let _ = self.spawner.spawn(task);
            }
            Action::Delay => {}
        }

        Box::pin(rx)
    }

    /// Feed some data into the cache.
    ///
    /// **NOTE: If the cache type is [NoCache], this function will not take
    /// effect. **
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub async fn feed_many<K, V>(&self, values: impl IntoIterator<Item = (K, V)>)
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let tid = TypeId::of::<(K, V)>();
        let mut entry = self
            .inner
            .requests
            .entry_async(tid)
            .await
            .or_insert_with(|| Box::new(Requests::<K, V, T>::new(&self.cache_factory)));

        let typed_requests = entry.downcast_mut::<Requests<K, V, T>>().unwrap();

        for (key, value) in values {
            typed_requests
                .cache_storage
                .insert(Cow::Owned(key), Cow::Owned(value));
        }
    }

    /// Feed some data into the cache.
    ///
    /// **NOTE: If the cache type is [NoCache], this function will not take
    /// effect. **
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub async fn feed_one<K, V>(&self, key: K, value: V)
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        self.feed_many(std::iter::once((key, value))).await;
    }

    /// Clear a specific entry from the cache.
    ///
    /// **NOTE: if the cache type is [NoCache], this function will not take
    /// effect. **
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub fn clear_one<K, V>(&self, key: &K)
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let tid = TypeId::of::<(K, V)>();
        let mut entry = self
            .inner
            .requests
            .entry_sync(tid)
            .or_insert_with(|| Box::new(Requests::<K, V, T>::new(&self.cache_factory)));

        let typed_requests = entry.downcast_mut::<Requests<K, V, T>>().unwrap();
        typed_requests.cache_storage.remove(key);
    }

    /// Clears the cache.
    ///
    /// **NOTE: If the cache type is [NoCache], this function will not take
    /// effect. **
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub fn clear<K, V>(&self)
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let tid = TypeId::of::<(K, V)>();
        let mut entry = self
            .inner
            .requests
            .entry_sync(tid)
            .or_insert_with(|| Box::new(Requests::<K, V, T>::new(&self.cache_factory)));

        let typed_requests = entry.downcast_mut::<Requests<K, V, T>>().unwrap();
        typed_requests.cache_storage.clear();
    }

    /// Gets all values in the cache.
    pub async fn get_cached_values<K, V>(&self) -> HashMap<K, V>
    where
        K: Send + Sync + Hash + Eq + Clone + 'static,
        V: Send + Sync + Clone + 'static,
        T: Loader<K, V>,
    {
        let tid = TypeId::of::<(K, V)>();
        match self.inner.requests.get_async(&tid).await {
            None => HashMap::new(),
            Some(requests) => {
                let typed_requests = requests.get().downcast_ref::<Requests<K, V, T>>().unwrap();
                typed_requests
                    .cache_storage
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures_util::{TryStreamExt, stream};
    use rustc_hash::FxBuildHasher;

    use super::*;
    use crate::runtime::{TokioSpawner, TokioTimer};

    struct MyLoader;

    #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
    impl Loader<i32, i32> for MyLoader {
        type Error = ();

        async fn load(&self, keys: &[i32]) -> Result<HashMap<i32, i32>, Self::Error> {
            assert!(keys.len() <= 10);
            Ok(keys.iter().copied().map(|k| (k, k)).collect())
        }
    }

    #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
    impl Loader<i64, i64> for MyLoader {
        type Error = ();

        async fn load(&self, keys: &[i64]) -> Result<HashMap<i64, i64>, Self::Error> {
            assert!(keys.len() <= 10);
            Ok(keys.iter().copied().map(|k| (k, k)).collect())
        }
    }

    #[tokio::test]
    async fn test_dataloader() {
        let loader = Arc::new(
            DataLoader::new(MyLoader, TokioSpawner::current(), TokioTimer::default())
                .max_batch_size(10),
        );
        assert_eq!(
            futures_util::future::try_join_all((0..100i32).map({
                let loader = loader.clone();
                move |n| {
                    let loader = loader.clone();
                    async move { loader.load_one(n).await }
                }
            }))
            .await
            .unwrap(),
            (0..100).map(Option::Some).collect::<Vec<_>>()
        );

        assert_eq!(
            futures_util::future::try_join_all((0..100i64).map({
                let loader = loader.clone();
                move |n| {
                    let loader = loader.clone();
                    async move { loader.load_one(n).await }
                }
            }))
            .await
            .unwrap(),
            (0..100).map(Option::Some).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_duplicate_keys() {
        let loader = Arc::new(
            DataLoader::new(MyLoader, TokioSpawner::current(), TokioTimer::default())
                .max_batch_size(10),
        );
        assert_eq!(
            futures_util::future::try_join_all([1, 3, 5, 1, 7, 8, 3, 7].iter().copied().map({
                let loader = loader.clone();
                move |n| {
                    let loader = loader.clone();
                    async move { loader.load_one(n).await }
                }
            }))
            .await
            .unwrap(),
            [1, 3, 5, 1, 7, 8, 3, 7]
                .iter()
                .copied()
                .map(Option::Some)
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_dataloader_load_empty() {
        let loader = DataLoader::new(MyLoader, TokioSpawner::current(), TokioTimer::default());
        assert!(loader.load_many::<i32, _>(vec![]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_dataloader_with_cache() {
        let loader = DataLoader::with_cache(
            MyLoader,
            TokioSpawner::current(),
            TokioTimer::default(),
            HashMapCache::default(),
        );
        loader.feed_many(vec![(1, 10), (2, 20), (3, 30)]).await;

        // All from the cache
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 10), (2, 20), (3, 30)].into_iter().collect()
        );

        // Part from the cache
        assert_eq!(
            loader.load_many(vec![1, 5, 6]).await.unwrap(),
            vec![(1, 10), (5, 5), (6, 6)].into_iter().collect()
        );

        // All from the loader
        assert_eq!(
            loader.load_many(vec![8, 9, 10]).await.unwrap(),
            vec![(8, 8), (9, 9), (10, 10)].into_iter().collect()
        );

        // Clear cache
        loader.clear::<i32, i32>();
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 1), (2, 2), (3, 3)].into_iter().collect()
        );
    }

    #[tokio::test]
    async fn test_dataloader_with_cache_hashmap_fx() {
        let loader = DataLoader::with_cache(
            MyLoader,
            TokioSpawner::current(),
            TokioTimer::default(),
            HashMapCache::<FxBuildHasher>::new(),
        );
        loader.feed_many(vec![(1, 10), (2, 20), (3, 30)]).await;

        // All from the cache
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 10), (2, 20), (3, 30)].into_iter().collect()
        );

        // Part from the cache
        assert_eq!(
            loader.load_many(vec![1, 5, 6]).await.unwrap(),
            vec![(1, 10), (5, 5), (6, 6)].into_iter().collect()
        );

        // All from the loader
        assert_eq!(
            loader.load_many(vec![8, 9, 10]).await.unwrap(),
            vec![(8, 8), (9, 9), (10, 10)].into_iter().collect()
        );

        // Clear cache
        loader.clear::<i32, i32>();
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 1), (2, 2), (3, 3)].into_iter().collect()
        );
    }

    #[tokio::test]
    async fn test_dataloader_disable_all_cache() {
        let loader = DataLoader::with_cache(
            MyLoader,
            TokioSpawner::current(),
            TokioTimer::default(),
            HashMapCache::default(),
        );
        loader.feed_many(vec![(1, 10), (2, 20), (3, 30)]).await;

        // All from the loader
        loader.enable_all_cache(false);
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 1), (2, 2), (3, 3)].into_iter().collect()
        );

        // All from the cache
        loader.enable_all_cache(true);
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 10), (2, 20), (3, 30)].into_iter().collect()
        );
    }

    #[tokio::test]
    async fn test_dataloader_evict_one_from_cache() {
        let loader = DataLoader::with_cache(
            MyLoader,
            TokioSpawner::current(),
            TokioTimer::default(),
            HashMapCache::default(),
        );
        loader.feed_many(vec![(1, 10), (2, 20), (3, 30)]).await;

        // All from the cache
        loader.enable_all_cache(true);
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 10), (2, 20), (3, 30)].into_iter().collect()
        );

        // Remove one from cache
        loader.clear_one(&1);
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 1), (2, 20), (3, 30)].into_iter().collect()
        );
    }

    #[tokio::test]
    async fn test_dataloader_disable_cache() {
        let loader = DataLoader::with_cache(
            MyLoader,
            TokioSpawner::current(),
            TokioTimer::default(),
            HashMapCache::default(),
        );
        loader.feed_many(vec![(1, 10), (2, 20), (3, 30)]).await;

        // All from the loader
        loader.enable_cache::<i32, i32>(false).await;
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 1), (2, 2), (3, 3)].into_iter().collect()
        );

        // All from the cache
        loader.enable_cache::<i32, i32>(true).await;
        assert_eq!(
            loader.load_many(vec![1, 2, 3]).await.unwrap(),
            vec![(1, 10), (2, 20), (3, 30)].into_iter().collect()
        );
    }

    struct StreamingLoader;

    #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
    impl Loader<i32, String> for StreamingLoader {
        type Error = ();

        async fn load(&self, keys: &[i32]) -> Result<HashMap<i32, String>, Self::Error> {
            Ok(keys
                .iter()
                .copied()
                .map(|k| (k, format!("value-{k}")))
                .collect())
        }

        #[cfg(feature = "boxed-trait")]
        fn load_stream<'a>(
            &'a self,
            keys: &'a [i32],
        ) -> LoaderStream<'a, i32, String, Self::Error> {
            Box::pin(stream::iter(
                keys.iter().copied().map(|k| Ok((k, format!("stream-{k}")))),
            ))
        }

        #[cfg(not(feature = "boxed-trait"))]
        fn load_stream<'a>(
            &'a self,
            keys: &'a [i32],
        ) -> impl Stream<Item = Result<(i32, String), Self::Error>> + Send + 'a {
            stream::iter(keys.iter().copied().map(|k| Ok((k, format!("stream-{k}")))))
        }
    }

    #[tokio::test]
    async fn test_dataloader_stream() {
        let loader = DataLoader::new(
            StreamingLoader,
            TokioSpawner::current(),
            TokioTimer::default(),
        );

        let result: HashMap<_, _> = loader
            .load_many_stream(vec![1, 2, 3])
            .await
            .try_collect()
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(&1), Some(&"stream-1".to_string()));
        assert_eq!(result.get(&2), Some(&"stream-2".to_string()));
        assert_eq!(result.get(&3), Some(&"stream-3".to_string()));
    }

    #[tokio::test]
    async fn test_dataloader_stream_with_cache() {
        let loader = DataLoader::with_cache(
            StreamingLoader,
            TokioSpawner::current(),
            TokioTimer::default(),
            HashMapCache::default(),
        );
        loader
            .feed_many(vec![
                (1, "cached-1".to_string()),
                (2, "cached-2".to_string()),
            ])
            .await;

        // Part from cache, part from stream
        let result: HashMap<_, _> = loader
            .load_many_stream(vec![1, 2, 3])
            .await
            .try_collect()
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(&1), Some(&"cached-1".to_string()));
        assert_eq!(result.get(&2), Some(&"cached-2".to_string()));
        assert_eq!(result.get(&3), Some(&"stream-3".to_string()));
    }

    #[tokio::test]
    async fn test_dataloader_load_and_stream_use_separate_batches() {
        let loader = Arc::new(
            DataLoader::new(
                StreamingLoader,
                TokioSpawner::current(),
                TokioTimer::default(),
            )
            .delay(Duration::from_millis(50)),
        );

        let normal_handle = tokio::spawn({
            let loader = loader.clone();
            async move { loader.load_many(vec![1]).await.unwrap() }
        });
        tokio::task::yield_now().await;

        let streamed: HashMap<_, _> = loader
            .load_many_stream(vec![2])
            .await
            .try_collect()
            .await
            .unwrap();
        let normal = normal_handle.await.unwrap();

        assert_eq!(normal.get(&1), Some(&"value-1".to_string()));
        assert_eq!(streamed.get(&2), Some(&"stream-2".to_string()));
    }

    #[tokio::test]
    async fn test_dataloader_same_key_different_value_types() {
        // This test demonstrates that loaders can be distinguished by return
        // type, not just by key type. Both implementations use i32 keys, but
        // they return different value types (i32 vs String).
        //
        // With the old TypeId::of::<K>() approach, loaders with the same key
        // type would share request map entries. With TypeId::of::<(K, V)>() they
        // are kept separate even within the same DataLoader type.
        struct MultiLoader;

        #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
        impl Loader<i32, i32> for MultiLoader {
            type Error = ();

            async fn load(&self, keys: &[i32]) -> Result<HashMap<i32, i32>, Self::Error> {
                Ok(keys.iter().copied().map(|k| (k, k * 10)).collect())
            }
        }

        #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
        impl Loader<i32, String> for MultiLoader {
            type Error = ();

            async fn load(&self, keys: &[i32]) -> Result<HashMap<i32, String>, Self::Error> {
                Ok(keys
                    .iter()
                    .copied()
                    .map(|k| (k, format!("value-{k}")))
                    .collect())
            }
        }

        let loader = DataLoader::new(MultiLoader, TokioSpawner::current(), TokioTimer::default());

        let int_result: Option<i32> = loader.load_one(42i32).await.unwrap();
        let str_result: Option<String> = loader.load_one(42i32).await.unwrap();

        assert_eq!(int_result, Some(420));
        assert_eq!(str_result, Some("value-42".to_string()));
    }

    #[tokio::test]
    async fn test_dataloader_dead_lock() {
        struct MyDelayLoader;

        #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
        impl Loader<i32, i32> for MyDelayLoader {
            type Error = ();

            async fn load(&self, keys: &[i32]) -> Result<HashMap<i32, i32>, Self::Error> {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok(keys.iter().copied().map(|k| (k, k)).collect())
            }
        }

        let loader = Arc::new(
            DataLoader::with_cache(
                MyDelayLoader,
                TokioSpawner::current(),
                TokioTimer::default(),
                NoCache,
            )
            .delay(Duration::from_secs(1)),
        );
        let handle = tokio::spawn({
            let loader = loader.clone();
            async move {
                loader.load_many(vec![1, 2, 3]).await.unwrap();
            }
        });

        tokio::time::sleep(Duration::from_millis(500)).await;
        handle.abort();
        loader.load_many(vec![4, 5, 6]).await.unwrap();
    }

    #[tokio::test]
    async fn test_dataloader_zero_delay() {
        struct CountingLoader {
            calls: Arc<AtomicUsize>,
        }

        #[cfg_attr(feature = "boxed-trait", async_trait::async_trait)]
        impl Loader<i32, i32> for CountingLoader {
            type Error = ();

            async fn load(&self, keys: &[i32]) -> Result<HashMap<i32, i32>, Self::Error> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(keys.iter().copied().map(|k| (k, k)).collect())
            }
        }

        // Verify that the default zero delay still batches concurrent requests.
        let calls = Arc::new(AtomicUsize::new(0));
        let loader = Arc::new(DataLoader::new(
            CountingLoader {
                calls: calls.clone(),
            },
            TokioSpawner::current(),
            TokioTimer::default(),
        ));
        assert_eq!(
            futures_util::future::try_join_all((0..10i32).map({
                let loader = loader.clone();
                move |n| {
                    let loader = loader.clone();
                    async move { loader.load_one(n).await }
                }
            }))
            .await
            .unwrap(),
            (0..10).map(Option::Some).collect::<Vec<_>>()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
