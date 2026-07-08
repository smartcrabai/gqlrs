use futures_util::TryStreamExt;
use gqlrs::*;

struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hello(&self) -> &str {
        "world"
    }
}

struct Mutation;

#[Object]
impl Mutation {
    async fn upload_stream(&self, ctx: &Context<'_>, file: Upload) -> Result<usize> {
        let value = file.value(ctx)?;
        let chunks: Vec<bytes::Bytes> = value.content_stream(1024).try_collect().await?;
        Ok(chunks.iter().map(|chunk| chunk.len()).sum())
    }
}

#[cfg(feature = "tempfile")]
fn make_upload_value(filename: &str, content_type: Option<&str>, data: &[u8]) -> UploadValue {
    use std::io::{Seek, SeekFrom, Write};

    let mut file = tempfile::tempfile().unwrap();
    file.write_all(data).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    UploadValue {
        filename: filename.to_string(),
        content_type: content_type.map(ToString::to_string),
        content: file,
    }
}

#[cfg(not(feature = "tempfile"))]
fn make_upload_value(filename: &str, content_type: Option<&str>, data: &[u8]) -> UploadValue {
    UploadValue {
        filename: filename.to_string(),
        content_type: content_type.map(ToString::to_string),
        content: bytes::Bytes::copy_from_slice(data),
    }
}

async fn collect_upload(upload: UploadValue, chunk_size: usize) -> Vec<bytes::Bytes> {
    upload
        .content_stream(chunk_size)
        .try_collect()
        .await
        .unwrap()
}

#[tokio::test]
async fn test_upload_content_stream() {
    let data = b"Hello, World! This is test data for streaming.";
    let upload = make_upload_value("test.txt", Some("text/plain"), data);

    let chunks = collect_upload(upload, 10).await;

    let mut reconstructed = Vec::new();
    for chunk in &chunks {
        reconstructed.extend_from_slice(chunk);
    }
    assert_eq!(reconstructed, data);
    assert!(!chunks.is_empty());
}

#[tokio::test]
async fn test_upload_content_stream_single_chunk() {
    let data = b"small";
    let upload = make_upload_value("small.txt", Some("text/plain"), data);

    let chunks = collect_upload(upload, 1024).await;

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].as_ref(), b"small");
}

#[tokio::test]
async fn test_upload_content_stream_zero_chunk_size() {
    let upload = make_upload_value("zero.txt", Some("text/plain"), b"abc");

    let chunks = collect_upload(upload, 0).await;

    assert_eq!(chunks.len(), 3);
    assert!(chunks.iter().all(|chunk| chunk.len() == 1));
}

#[tokio::test]
async fn test_upload_content_stream_empty() {
    let upload = make_upload_value("empty.txt", Some("text/plain"), b"");

    let chunks = collect_upload(upload, 1024).await;

    assert!(chunks.is_empty());
}

#[tokio::test]
async fn test_upload_content_stream_in_resolver() {
    let data = b"streamed through resolver";
    let schema = Schema::new(QueryRoot, Mutation, EmptySubscription);
    let mut request = Request::new("mutation($file: Upload!) { uploadStream(file: $file) }")
        .variables(Variables::from_value(value!({ "file": null })));
    request.set_upload(
        "variables.file",
        make_upload_value("resolver.txt", Some("text/plain"), data),
    );

    let response = schema.execute(request).await;

    assert!(response.errors.is_empty());
    assert_eq!(response.data, value!({ "uploadStream": data.len() }));
}
