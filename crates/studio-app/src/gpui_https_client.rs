//! Production `studio-net` adapter over the GPUI platform HTTP client.

use std::sync::{Arc, OnceLock, mpsc};

use futures::AsyncReadExt;
use gpui_kit::http_client::{AsyncBody, HttpClient, HttpRequestExt, RedirectPolicy, Request, Url};
use studio_net::transport::ByteStream;
use studio_net::{HttpsClient, IncomingResponse, OutgoingRequest, TransportError, TransportLimits};
use zeroize::Zeroize;

/// A lazily installed GPUI HTTP client usable by the synchronous host network seam.
///
/// `studio-app` prepares a verified package before opening its window. The platform HTTP client is
/// available once GPUI starts, so this adapter is created before launch and initialized from the
/// app context before any guest action can use it.
#[derive(Clone, Default)]
pub(crate) struct GpuiHttpsClient {
    client: Arc<OnceLock<Arc<dyn HttpClient>>>,
}

impl std::fmt::Debug for GpuiHttpsClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GpuiHttpsClient(..)")
    }
}

impl GpuiHttpsClient {
    pub(crate) fn install(&self, client: Arc<dyn HttpClient>) -> bool {
        self.client.set(client).is_ok()
    }

    fn client(&self) -> Result<&Arc<dyn HttpClient>, TransportError> {
        self.client.get().ok_or(TransportError::ConnectionFailure)
    }
}

impl HttpsClient for GpuiHttpsClient {
    fn execute(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        if !limits.is_valid() || !valid_https_url(&request.url) {
            return Err(TransportError::ConnectionFailure);
        }
        let timeout = limits
            .connect_timeout
            .saturating_add(limits.write_timeout)
            .saturating_add(limits.read_timeout);
        let mut builder = Request::builder()
            .method(request.method.as_str())
            .uri(request.url)
            .timeout(timeout)
            .follow_redirects(RedirectPolicy::NoFollow);
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        let body = request.body.map_or_else(AsyncBody::empty, AsyncBody::from);
        let request = builder
            .body(body)
            .map_err(|_| TransportError::ConnectionFailure)?;
        let response = futures::executor::block_on(self.client()?.send(request))
            .map_err(|_| TransportError::ConnectionFailure)?;
        let status = response.status().as_u16();
        let media_type = response
            .headers()
            .get(gpui_kit::http_client::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        let read = futures::executor::block_on(async {
            response
                .into_body()
                .take(
                    u64::try_from(limits.max_response_bytes.saturating_add(1)).unwrap_or(u64::MAX),
                )
                .read_to_end(&mut body)
                .await
        });
        if read.is_err() {
            body.zeroize();
            return Err(TransportError::ConnectionFailure);
        }
        if body.len() > limits.max_response_bytes {
            body.zeroize();
            return Err(TransportError::BodyTooLarge);
        }
        Ok(IncomingResponse {
            status,
            media_type,
            body,
        })
    }

    fn open_stream(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<Box<dyn ByteStream>, TransportError> {
        if !limits.is_valid() || !valid_https_url(&request.url) {
            return Err(TransportError::ConnectionFailure);
        }
        let client = Arc::clone(self.client()?);
        let timeout = limits
            .connect_timeout
            .saturating_add(limits.write_timeout)
            .saturating_add(limits.read_timeout);
        let mut builder = Request::builder()
            .method(request.method.as_str())
            .uri(request.url)
            .timeout(timeout)
            .follow_redirects(RedirectPolicy::NoFollow);
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        let body = request.body.map_or_else(AsyncBody::empty, AsyncBody::from);
        let request = builder
            .body(body)
            .map_err(|_| TransportError::ConnectionFailure)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker_sender = sender.clone();
        std::thread::Builder::new()
            .name(String::from("studio-https-stream"))
            .spawn(move || {
                let outcome = futures::executor::block_on(async move {
                    let response = client
                        .send(request)
                        .await
                        .map_err(|_| TransportError::ConnectionFailure)?;
                    if !response.status().is_success() {
                        return Err(TransportError::ConnectionFailure);
                    }
                    let mut body = response.into_body();
                    let mut total = 0_usize;
                    loop {
                        let chunk_size = limits.max_stream_chunk_bytes.min(64 * 1024).max(1);
                        let mut chunk = vec![0_u8; chunk_size];
                        let count = match body.read(&mut chunk).await {
                            Ok(count) => count,
                            Err(_) => {
                                chunk.zeroize();
                                return Err(TransportError::ConnectionFailure);
                            }
                        };
                        if count == 0 {
                            let _ = worker_sender.send(Ok(None));
                            return Ok(());
                        }
                        if count > limits.max_stream_bytes.saturating_sub(total) {
                            chunk.zeroize();
                            return Err(TransportError::BodyTooLarge);
                        }
                        total = total.saturating_add(count);
                        chunk.truncate(count);
                        if worker_sender.send(Ok(Some(chunk))).is_err() {
                            return Ok(());
                        }
                    }
                });
                if let Err(error) = outcome {
                    let _ = sender.send(Err(error));
                }
            })
            .map_err(|_| TransportError::ConnectionFailure)?;
        Ok(Box::new(GpuiByteStream { receiver }))
    }
}

struct GpuiByteStream {
    receiver: mpsc::Receiver<Result<Option<Vec<u8>>, TransportError>>,
}

impl ByteStream for GpuiByteStream {
    fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        self.receiver
            .recv()
            .map_err(|_| TransportError::ConnectionFailure)?
    }
}

fn valid_https_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}
