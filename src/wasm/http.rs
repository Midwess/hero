//! Browser-native request construction with Fetch and XMLHttpRequest backends.
//!
//! Builder methods consume and return [`HttpClient`], allowing a request to be
//! assembled before selecting a transport. Dropping an in-flight
//! [`FetchRequest`] aborts it; dropping [`XmlHttpRequest`] calls the browser's
//! abort method.

use futures::channel::mpsc::{channel, Receiver};
use futures::{AsyncRead, StreamExt};
use futures_util::SinkExt;
use js_sys::{Object, Reflect, Uint8Array};
use serde::Serialize;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::{Closure, JsValue};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use wasm_streams::ReadableStream;
use web_sys::{
    AbortController, Blob, Event, Headers, ProgressEvent, Request, RequestInit, Response,
    XmlHttpRequest as WasmXmlHttpRequest,
};

/// Transport-neutral browser HTTP request builder.
pub struct HttpClient {
    method: String,
    url: Option<String>,
    headers: HashMap<String, String>,
    body: Option<Body>,
    stream_buffer_size: usize,
    reject_redirects: bool,
}

/// Request body formats supported by Hero's browser transports.
pub enum Body {
    /// UTF-8 text passed directly to the browser.
    Text(String),
    /// Browser-managed binary blob.
    Blob(Blob),
    /// Rust-owned bytes copied into a `Uint8Array` when Fetch is selected.
    Bytes(Vec<u8>),
    /// Existing JavaScript byte array.
    Uint8Array(Uint8Array),
    /// Streaming body available only through Fetch in supporting browsers.
    AsyncReader(Box<dyn AsyncRead + Unpin + 'static>),
}

/// Started XMLHttpRequest with a bounded stream of progress and completion events.
pub struct XmlHttpRequest {
    xml_http: Rc<WasmXmlHttpRequest>,
    event_rx: Receiver<XhrEvent>,
}

/// Prepared Fetch request that aborts when dropped before completion.
pub struct FetchRequest {
    request: Request,
    _abort_controller: Option<AbortController>,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    /// Starts a `GET` request with no URL, headers, or body.
    pub fn new() -> Self {
        Self {
            method: "GET".to_string(),
            url: None,
            headers: HashMap::new(),
            body: None,
            stream_buffer_size: 1024 * 1024,
            reject_redirects: false,
        }
    }

    /// Sets the uppercase HTTP method.
    pub fn method(mut self, method: &str) -> Self {
        self.method = method.to_uppercase();
        self
    }

    /// Sets the absolute or browser-resolvable request URL.
    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    /// Makes Fetch fail on redirects, preserving the signed endpoint boundary.
    ///
    /// The default follows the browser's redirect behavior. This option applies
    /// to Fetch; XMLHttpRequest does not expose a redirect policy.
    pub fn reject_redirects(mut self) -> Self {
        self.reject_redirects = true;
        self
    }

    /// Adds or replaces one request header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
        self
    }

    /// Uses a copied text body.
    pub fn body_text(mut self, text: &str) -> Self {
        self.body = Some(Body::Text(text.to_string()));
        self
    }

    /// Uses an existing browser blob as the body.
    pub fn body_blob(mut self, blob: Blob) -> Self {
        self.body = Some(Body::Blob(blob));
        self
    }

    /// Uses owned Rust bytes as the body.
    pub fn body_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.body = Some(Body::Bytes(bytes));
        self
    }

    /// Uses an existing JavaScript typed array as the body.
    pub fn body_uint8array(mut self, u8a: Uint8Array) -> Self {
        self.body = Some(Body::Uint8Array(u8a));
        self
    }

    /// Sets a body variant directly.
    pub fn body(mut self, body: Body) -> Self {
        self.body = Some(body);
        self
    }

    /// Detects the browser's Fetch request-streaming extension.
    ///
    /// Browser support is inferred from the presence of `Request.prototype.duplex`.
    pub fn is_support_duplex_stream() -> bool {
        let global = js_sys::global();
        let Ok(request_ctor) = Reflect::get(&global, &JsValue::from_str("Request")) else {
            return false;
        };
        let Ok(proto) = Reflect::get(&request_ctor, &JsValue::from_str("prototype")) else {
            return false;
        };
        Reflect::has(&proto, &JsValue::from_str("duplex")).unwrap_or(false)
    }

    /// Streams an async reader through Fetch using `duplex: "half"`.
    ///
    /// Browsers without request-streaming support are rejected during request
    /// construction. XMLHttpRequest cannot send this body variant.
    pub fn body_stream(mut self, value: impl AsyncRead + Unpin + 'static) -> Result<Self, anyhow::Error> {
        if !Self::is_support_duplex_stream() {
            return Err(anyhow::anyhow!("Browser does not support duplex stream"));
        }

        self.body = Some(Body::AsyncReader(Box::new(value)));
        Ok(self)
    }

    /// Bounds the Rust reader buffer used by a streaming Fetch upload.
    ///
    /// Zero is rejected; this does not bound buffering inside the browser.
    pub fn stream_buffer_size(mut self, bytes: usize) -> Result<Self, anyhow::Error> {
        anyhow::ensure!(bytes > 0, "stream buffer size must be positive");
        self.stream_buffer_size = bytes;
        Ok(self)
    }

    /// Serializes JSON and sets `Content-Type: application/json`.
    pub fn body_json<T: Serialize>(mut self, value: &T) -> Result<Self, serde_json::Error> {
        let json_string = serde_json::to_string(value)?;
        self.headers.insert("Content-Type".into(), "application/json".into());
        self.body = Some(Body::Text(json_string));
        Ok(self)
    }

    /// Starts an XMLHttpRequest immediately and returns its event stream.
    ///
    /// XHR is the appropriate backend when upload progress is needed. Streaming
    /// reader bodies are rejected.
    pub fn xhr(self) -> Result<XmlHttpRequest, JsValue> {
        let url = self.url.ok_or_else(|| JsValue::from_str("URL must be set before send()"))?;
        let xml_http = Rc::new(WasmXmlHttpRequest::new()?);
        xml_http.open_with_async(&self.method, &url, true)?;

        for (k, v) in &self.headers {
            xml_http.set_request_header(k, v)?;
        }

        // We want small buffer to avoid any overhead
        // the consumer must consuming fast.
        let (tx, rx) = channel::<XhrEvent>(8);

        let mut tx1 = tx.clone();
        let progress_closure = Closure::wrap(Box::new(move |e: ProgressEvent| {
            let _ = tx1.try_send(XhrEvent::InProgress(e));
        }) as Box<dyn FnMut(_)>);
        let x_upload = xml_http.upload()?;
        x_upload.set_onprogress(Some(progress_closure.as_ref().unchecked_ref()));
        progress_closure.forget();

        let tx2 = tx.clone();
        let xhr_clone = xml_http.clone();
        let onload_closure = Closure::wrap(Box::new(move |_: Event| {
            let mut headers_map = HashMap::new();
            if let Ok(headers_str) = xhr_clone.get_all_response_headers() {
                headers_map = parse_headers_to_hashmap(&headers_str);
            }
            let body = xhr_clone.response().unwrap_or(JsValue::NULL);
            let mut tx2 = tx2.clone();
            spawn_local(async move {
                let _ = tx2
                    .send(XhrEvent::Complete {
                        headers: headers_map,
                        body,
                    })
                    .await;
            });
        }) as Box<dyn FnMut(_)>);
        xml_http.set_onload(Some(onload_closure.as_ref().unchecked_ref()));
        onload_closure.forget();

        let tx3 = tx.clone();
        let onerror_closure = Closure::wrap(Box::new(move |_: Event| {
            let mut tx3 = tx3.clone();
            spawn_local(async move {
                let _ = tx3.send(XhrEvent::Error(JsValue::from_str("Network error"))).await;
            });
        }) as Box<dyn FnMut(_)>);
        xml_http.set_onerror(Some(onerror_closure.as_ref().unchecked_ref()));
        onerror_closure.forget();

        let tx4 = tx.clone();
        let onabort_closure = Closure::wrap(Box::new(move |_: Event| {
            let mut tx4 = tx4.clone();
            spawn_local(async move {
                let _ = tx4.send(XhrEvent::Error(JsValue::from_str("Aborted"))).await;
            });
        }) as Box<dyn FnMut(_)>);
        xml_http.set_onabort(Some(onabort_closure.as_ref().unchecked_ref()));
        onabort_closure.forget();

        match self.body {
            Some(Body::Text(s)) => xml_http.send_with_opt_str(Some(&s))?,
            Some(Body::Blob(b)) => xml_http.send_with_opt_blob(Some(&b))?,
            Some(Body::Bytes(ref bytes)) => xml_http.send_with_opt_u8_array(Some(bytes))?,
            Some(Body::Uint8Array(u8a)) => xml_http.send_with_opt_js_u8_array(Some(&u8a))?,
            Some(Body::AsyncReader(_)) => return Err(JsValue::from_str("Async reader not supported for xhr")),
            None => xml_http.send()?,
        };

        Ok(XmlHttpRequest { xml_http, event_rx: rx })
    }

    /// Prepares a Fetch request without sending it.
    ///
    /// The returned value owns an `AbortController`; dropping it aborts a fetch
    /// that is still pending.
    pub fn fetch(self) -> Result<FetchRequest, JsValue> {
        let url = self.url.ok_or_else(|| JsValue::from_str("URL must be set before fetch()"))?;

        let abort_controller = Some(AbortController::new()?);

        let opts = RequestInit::new();
        opts.set_method(&self.method);
        if self.reject_redirects {
            opts.set_redirect(web_sys::RequestRedirect::Error);
        }

        if let Some(ref controller) = abort_controller {
            opts.set_signal(Some(&controller.signal()))
        }

        let headers = Headers::new()?;
        for (k, v) in &self.headers {
            headers.append(k, v)?;
        }
        opts.set_headers(&headers);

        // Set up body appropriately
        if let Some(body) = self.body {
            match body {
                Body::Text(s) => {
                    opts.set_body(&JsValue::from_str(&s));
                }
                Body::Blob(b) => {
                    opts.set_body(b.as_ref());
                }
                Body::Bytes(vec) => {
                    let u8array = Uint8Array::from(vec.as_slice());
                    opts.set_body(&u8array);
                }
                Body::Uint8Array(u8a) => {
                    opts.set_body(&u8a);
                }
                Body::AsyncReader(reader) => {
                    let readable_stream = ReadableStream::from_async_read(reader, self.stream_buffer_size);
                    let init_obj: &Object = opts.as_ref();
                    Reflect::set(init_obj, &JsValue::from_str("duplex"), &JsValue::from_str("half"))?;
                    opts.set_body(readable_stream.as_raw());
                }
            }
        }

        let request = Request::new_with_str_and_init(&url, &opts)?;

        Ok(FetchRequest {
            request,
            _abort_controller: abort_controller,
        })
    }
}

fn parse_headers(headers: web_sys::Headers) -> Result<HashMap<String, String>, JsValue> {
    let entries =
        js_sys::try_iter(&headers.entries())?.ok_or_else(|| JsValue::from_str("Failed to iterate headers"))?;
    let mut headers_map = HashMap::new();
    for entry in entries {
        let entry = entry?;
        let pair: js_sys::Array = entry.dyn_into().map_err(|_| JsValue::from_str("Invalid header entry"))?;
        let key = pair.get(0).as_string().unwrap_or_default();
        let value = pair.get(1).as_string().unwrap_or_default();
        headers_map.insert(key, value);
    }
    Ok(headers_map)
}

#[derive(Debug)]
/// Upload lifecycle events emitted by [`XmlHttpRequest`].
pub enum XhrEvent {
    /// Browser upload progress event.
    InProgress(ProgressEvent),
    /// Request completed with parsed response headers and the raw response body.
    Complete {
        /// Response headers keyed by browser-normalized names.
        headers: HashMap<String, String>,
        /// Raw XMLHttpRequest response value.
        body: JsValue,
    },
    /// Network failure or explicit abort.
    Error(JsValue),
}

impl XmlHttpRequest {
    /// Waits for the next upload or completion event.
    pub async fn next_event(&mut self) -> Option<XhrEvent> {
        self.event_rx.next().await
    }

    /// Waits for completion while discarding progress events.
    pub async fn response(mut self) -> Result<(HashMap<String, String>, JsValue), JsValue> {
        while let Some(event) = self.next_event().await {
            match event {
                XhrEvent::Complete { headers, body } => return Ok((headers, body)),
                XhrEvent::Error(err) => return Err(err),
                XhrEvent::InProgress(_) => {
                    // Ignore progress events here or handle if desired
                }
            }
        }

        Err(JsValue::from_str("Response channel closed"))
    }
}

impl Drop for XmlHttpRequest {
    fn drop(&mut self) {
        let _ = self.xml_http.abort();
    }
}

type ResponseChunks = std::pin::Pin<Box<dyn futures::Stream<Item = Result<JsValue, JsValue>>>>;

/// Fetch response that owns its abort controller until body consumption finishes.
///
/// Status and headers are available before the first body read. Each read pulls
/// one browser chunk; no response pump or complete-body allocation is created.
/// Dropping the response aborts outstanding network and body work.
pub struct StreamingResponse {
    request: FetchRequest,
    response: Response,
    body: Option<ResponseChunks>,
}

impl StreamingResponse {
    /// Returns a worker-local callback that aborts outstanding Fetch and body work.
    ///
    /// The callback can be attached to a cancellation observer without moving the
    /// response or polling its body. Calling it after EOF has no network effect.
    pub fn abort_handle(&self) -> impl FnOnce() + use<> {
        let controller = self.request._abort_controller.clone();
        move || {
            if let Some(controller) = controller {
                controller.abort();
            }
        }
    }

    /// Returns the HTTP status, including unsuccessful HTTP responses.
    pub fn status(&self) -> u16 {
        self.response.status()
    }

    /// Reads browser-visible headers without consuming the body.
    pub fn headers(&self) -> Result<HashMap<String, String>, JsValue> {
        parse_headers(self.response.headers())
    }

    /// Pulls one chunk, copying its browser buffer into owned Rust memory.
    ///
    /// A transport failure is returned unchanged. At EOF the abort controller
    /// is released without aborting the completed request.
    pub async fn next_chunk(&mut self) -> Result<Option<bytes::Bytes>, JsValue> {
        let Some(body) = self.body.as_mut() else {
            return Ok(None);
        };
        match body.next().await {
            Some(Ok(chunk)) => Ok(Some(Uint8Array::new(&chunk).to_vec().into())),
            Some(Err(error)) => Err(error),
            None => {
                self.body = None;
                self.request._abort_controller.take();
                Ok(None)
            }
        }
    }
}

impl FetchRequest {
    async fn fetch_response(&self) -> Result<Response, JsValue> {
        let global = js_sys::global();
        let fetch: js_sys::Function = Reflect::get(&global, &JsValue::from_str("fetch"))?.dyn_into()?;
        let promise: js_sys::Promise = fetch.call1(&global, &self.request)?.dyn_into()?;
        JsFuture::from(promise).await?.dyn_into()
    }

    /// Sends from the current window or worker and retains body cancellation.
    pub async fn send_streaming(self) -> Result<StreamingResponse, JsValue> {
        let response = self.fetch_response().await?;
        let body = response.body().map(|raw| Box::pin(ReadableStream::from_raw(raw).into_stream()) as ResponseChunks);
        Ok(StreamingResponse {
            request: self,
            response,
            body,
        })
    }

    /// Sends from the current window or worker without status checks.
    ///
    /// Ownership of body cancellation passes to the caller's browser response.
    /// Use `send_streaming` to abort body work when the Rust owner is dropped.
    pub async fn send(mut self) -> Result<Response, JsValue> {
        let response = self.fetch_response().await?;
        self._abort_controller.take();
        Ok(response)
    }

    /// Sends and buffers the complete body while retaining abort ownership.
    pub async fn bytes(self) -> Result<(u16, HashMap<String, String>, Vec<u8>), JsValue> {
        let response = self.fetch_response().await?;
        let status = response.status();
        let headers = parse_headers(response.headers())?;
        let buffer = JsFuture::from(response.array_buffer()?).await?;
        Ok((status, headers, Uint8Array::new(&buffer).to_vec()))
    }

    /// Decodes successful JSON; an empty or invalid JSON body becomes `NULL`.
    ///
    /// Non-2xx responses are errors. Cancellation ownership lasts through parsing.
    pub async fn response(self) -> Result<(HashMap<String, String>, JsValue), JsValue> {
        let response = self.fetch_response().await?;
        if !(200..300).contains(&response.status()) {
            return Err(JsValue::from_str(&format!("HTTP error: status {}", response.status())));
        }
        let headers = parse_headers(response.headers())?;
        let body = match response.json() {
            Ok(promise) => JsFuture::from(promise).await.unwrap_or(JsValue::NULL),
            Err(_) => JsValue::NULL,
        };
        Ok((headers, body))
    }
}

impl Drop for FetchRequest {
    fn drop(&mut self) {
        // Abort the request if it's still pending
        if let Some(controller) = &self._abort_controller {
            controller.abort();
        }
    }
}

fn parse_headers_to_hashmap(headers_str: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in headers_str.split("\r\n") {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}
