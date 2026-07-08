//! Relay persisted queries extension.

use std::sync::Arc;

use async_graphql_parser::types::ExecutableDocument;
use sha2::{Digest, Sha256};

use crate::{
    Request, ServerError, ServerResult, Value,
    extensions::{Extension, ExtensionContext, ExtensionFactory, NextPrepareRequest},
};

/// Cache storage for persisted queries.
///
/// This trait has the same API as Apollo's persisted-query cache storage, so a
/// custom backend can implement both traits when serving both Apollo and Relay
/// persisted queries.
#[async_trait::async_trait]
pub trait CacheStorage: Send + Sync + Clone + 'static {
    /// Load the query by `key`.
    async fn get(&self, key: String) -> Option<ExecutableDocument>;

    /// Save the query by `key`.
    async fn set(&self, key: String, query: ExecutableDocument);
}

/// Memory-based LRU cache.
#[derive(Clone)]
pub struct LruCacheStorage(Arc<scc::HashCache<String, ExecutableDocument>>);

impl LruCacheStorage {
    /// Creates a new LRU Cache that holds at most `cap` items.
    pub fn new(cap: usize) -> Self {
        Self(Arc::new(scc::HashCache::with_capacity(0, cap)))
    }
}

#[async_trait::async_trait]
impl CacheStorage for LruCacheStorage {
    async fn get(&self, key: String) -> Option<ExecutableDocument> {
        self.0
            .get_async(&key)
            .await
            .map(|entry| entry.get().clone())
    }

    async fn set(&self, key: String, query: ExecutableDocument) {
        let _ = self.0.put_async(key, query).await;
    }
}

/// Relay persisted queries extension.
///
/// [Reference](https://relay.dev/docs/guides/persisted-queries/)
#[cfg_attr(docsrs, doc(cfg(feature = "relay_persisted_queries")))]
pub struct RelayPersistedQueries<T>(T);

impl<T: CacheStorage> RelayPersistedQueries<T> {
    /// Creates a Relay persisted queries extension.
    pub fn new(cache_storage: T) -> RelayPersistedQueries<T> {
        Self(cache_storage)
    }
}

impl<T: CacheStorage> ExtensionFactory for RelayPersistedQueries<T> {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(RelayPersistedQueriesExtension {
            storage: self.0.clone(),
        })
    }
}

struct RelayPersistedQueriesExtension<T> {
    storage: T,
}

#[async_trait::async_trait]
impl<T: CacheStorage> Extension for RelayPersistedQueriesExtension<T> {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        mut request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let res = if let Some(document_id) = request.extensions.remove("documentId") {
            let document_id = match document_id {
                Value::String(s) => s,
                _ => {
                    return Err(ServerError::new(
                        "Invalid \"documentId\" extension value, expected string.",
                        None,
                    ));
                }
            };

            if request.query.is_empty() {
                // No query provided: look up by documentId
                if let Some(doc) = self.storage.get(document_id).await {
                    Ok(Request {
                        parsed_query: Some(doc),
                        ..request
                    })
                } else {
                    Err(ServerError::new("PersistedQueryNotFound", None))
                }
            } else {
                // Query provided: validate hash and cache
                let sha256_hash = format!("{:x}", Sha256::digest(request.query.as_bytes()));

                if document_id != sha256_hash {
                    Err(ServerError::new(
                        "provided documentId does not match query",
                        None,
                    ))
                } else {
                    let doc = async_graphql_parser::parse_query(&request.query)?;
                    self.storage.set(document_id, doc.clone()).await;
                    Ok(Request {
                        query: String::new(),
                        parsed_query: Some(doc),
                        ..request
                    })
                }
            }
        } else {
            Ok(request)
        };
        next.run(ctx, res?).await
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test() {
        use super::*;
        use crate::*;

        struct Query;

        #[Object(internal)]
        impl Query {
            async fn value(&self) -> i32 {
                100
            }
        }

        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(RelayPersistedQueries::new(LruCacheStorage::new(256)))
            .finish();

        // Register the query by sending both query and documentId
        let mut request = Request::new("{ value }");
        request.extensions.insert(
            "documentId".to_string(),
            value!("854174ebed716fe24fd6659c30290aecd9bc1d17dc4f47939a1848a1b8ed3c6b"),
        );

        assert_eq!(
            schema.execute(request).await.into_result().unwrap().data,
            value!({
                "value": 100
            })
        );

        // Retrieve by documentId only
        let mut request = Request::new("");
        request.extensions.insert(
            "documentId".to_string(),
            value!("854174ebed716fe24fd6659c30290aecd9bc1d17dc4f47939a1848a1b8ed3c6b"),
        );

        assert_eq!(
            schema.execute(request).await.into_result().unwrap().data,
            value!({
                "value": 100
            })
        );

        // Not found
        let mut request = Request::new("");
        request
            .extensions
            .insert("documentId".to_string(), value!("def"));

        assert_eq!(
            schema.execute(request).await.into_result().unwrap_err(),
            vec![ServerError::new("PersistedQueryNotFound", None)]
        );
    }

    #[tokio::test]
    async fn test_hash_mismatch() {
        use super::*;
        use crate::*;

        struct Query;

        #[Object(internal)]
        impl Query {
            async fn value(&self) -> i32 {
                100
            }
        }

        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(RelayPersistedQueries::new(LruCacheStorage::new(256)))
            .finish();

        // Hash mismatch: documentId does not match the query hash
        let mut request = Request::new("{ value }");
        request.extensions.insert(
            "documentId".to_string(),
            value!("0000000000000000000000000000000000000000000000000000000000000000"),
        );

        assert_eq!(
            schema.execute(request).await.into_result().unwrap_err(),
            vec![ServerError::new(
                "provided documentId does not match query",
                None
            )]
        );
    }

    #[tokio::test]
    async fn test_no_document_id_passthrough() {
        use super::*;
        use crate::*;

        struct Query;

        #[Object(internal)]
        impl Query {
            async fn value(&self) -> i32 {
                100
            }
        }

        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .extension(RelayPersistedQueries::new(LruCacheStorage::new(256)))
            .finish();

        // Normal request without documentId should pass through
        let request = Request::new("{ value }");

        assert_eq!(
            schema.execute(request).await.into_result().unwrap().data,
            value!({
                "value": 100
            })
        );
    }
}
