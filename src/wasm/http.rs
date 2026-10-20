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

        let abort_controller = AbortController::new().ok();

        let opts = RequestInit::new();
        opts.set_method(&self.method);

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
                    let readable_stream = ReadableStream::from_async_read(reader, 1024 * 1024);
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

impl FetchRequest {
    /// Sends the request and returns the browser response without status checks.
    pub async fn send(self) -> Result<Response, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No global window"))?;
        let resp_js = JsFuture::from(window.fetch_with_request(&self.request)).await?;
        resp_js.dyn_into().map_err(|_| JsValue::from_str("Failed to convert to Response"))
    }

    /// Sends the request and buffers the complete response body as bytes.
    ///
    /// The status code is returned to the caller rather than interpreted.
    pub async fn bytes(self) -> Result<(u16, HashMap<String, String>, Vec<u8>), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No global window"))?;
        let resp_js = JsFuture::from(window.fetch_with_request(&self.request)).await?;
        let resp: Response = resp_js.dyn_into().map_err(|_| JsValue::from_str("Failed to convert to Response"))?;

        let status = resp.status();
        let headers_map = parse_headers(resp.headers())?;

        let array_buffer_promise = resp.array_buffer()?;
        let array_buffer_js = JsFuture::from(array_buffer_promise).await?;
        let array_buffer: js_sys::ArrayBuffer =
            array_buffer_js.dyn_into().map_err(|_| JsValue::from_str("Failed to convert to ArrayBuffer"))?;
        let bytes = Uint8Array::new(&array_buffer).to_vec();

        Ok((status, headers_map, bytes))
    }

    /// Sends the request and decodes a successful response as JavaScript JSON.
    ///
    /// Non-2xx statuses are errors. If the body is empty or invalid JSON, the
    /// returned body is `JsValue::NULL` while response headers remain available.
    pub async fn response(self) -> Result<(HashMap<String, String>, JsValue), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No global window"))?;
        let resp_js = JsFuture::from(window.fetch_with_request(&self.request)).await?;
        let resp: Response = resp_js.dyn_into().map_err(|_| JsValue::from_str("Failed to convert to Response"))?;

        // Check status code
        let status = resp.status();
        if !(200..300).contains(&status) {
            return Err(JsValue::from_str(&format!("HTTP error: status {}", status)));
        }

        let headers_map = parse_headers(resp.headers())?;

        let Ok(json_promise) = resp.json().map(JsFuture::from) else {
            return Ok((headers_map, JsValue::NULL));
        };

        let Ok(json_js) = json_promise.await else {
            return Ok((headers_map, JsValue::NULL));
        };

        Ok((headers_map, json_js))
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
