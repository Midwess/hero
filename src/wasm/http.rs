use std::collections::HashMap;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::{Blob, XmlHttpRequest as WasmXmlHttpRequest, Event, ProgressEvent, AbortController};
use js_sys::Uint8Array;
use futures::channel::mpsc::{channel, Receiver};
use futures::StreamExt;
use futures_util::SinkExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::utils::never_send::NeverSend;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Request, RequestInit, Response, Headers};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::prelude::JsValue;

pub struct HttpClient {
    method: String,
    url: Option<String>,
    headers: HashMap<String, String>,
    body: Option<Body>,
}

#[derive(Clone)]
pub enum Body {
    Text(String),
    Blob(Blob),
    Bytes(Vec<u8>),
    Uint8Array(Uint8Array),
}

pub struct XmlHttpRequest {
    xml_http: Arc<NeverSend<WasmXmlHttpRequest>>,
    event_rx: Receiver<XhrEvent>,
}

pub struct FetchRequest {
    request: Request,
    _abort_controller: Option<AbortController>,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            method: "GET".to_string(),
            url: None,
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn method(mut self, method: &str) -> Self {
        self.method = method.to_uppercase();
        self
    }

    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
        self
    }

    pub fn body_text(mut self, text: &str) -> Self {
        self.body = Some(Body::Text(text.to_string()));
        self
    }

    pub fn body_blob(mut self, blob: Blob) -> Self {
        self.body = Some(Body::Blob(blob));
        self
    }

    pub fn body_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.body = Some(Body::Bytes(bytes));
        self
    }

    pub fn body_uint8array(mut self, u8a: Uint8Array) -> Self {
        self.body = Some(Body::Uint8Array(u8a));
        self
    }

    pub fn body_json<T: Serialize>(mut self, value: &T) -> Self {
        let json_string = serde_json::to_string(value).expect("Failed to serialize JSON");
        self.headers.insert("Content-Type".into(), "application/json".into());
        self.body = Some(Body::Text(json_string));
        self
    }

    pub fn xhr(self) -> Result<XmlHttpRequest, JsValue> {
        let url = self.url.expect("URL must be set before send()");
        let xml_http = Arc::new(NeverSend(WasmXmlHttpRequest::new()?));
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
        let x_upload = xml_http.upload().unwrap();
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
                let _ = tx2.send(XhrEvent::Complete {
                    headers: headers_map,
                    body,
                }).await;
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
            None => xml_http.send()?
        };

        Ok(XmlHttpRequest {
            xml_http,
            event_rx: rx,
        })
    }

    pub fn fetch(self) -> Result<FetchRequest, JsValue> {
        let url = self.url.expect("URL must be set before fetch()");

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
        if let Some(body) = &self.body {
            match body {
                Body::Text(s) => {
                    opts.set_body(&JsValue::from_str(s));
                }
                Body::Blob(b) => {
                    opts.set_body(b.as_ref());
                }
                Body::Bytes(vec) => {
                    let u8array = Uint8Array::from(vec.as_slice());
                    opts.set_body(&u8array);
                }
                Body::Uint8Array(u8a) => {
                    opts.set_body(u8a);
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

#[derive(Debug)]
pub enum XhrEvent {
    InProgress(ProgressEvent),
    Complete {
        headers: HashMap<String, String>,
        body: JsValue,
    },
    Error(JsValue),
}

impl XmlHttpRequest {
    pub async fn next_event(&mut self) -> Option<XhrEvent> {
        self.event_rx.next().await
    }

    pub async fn response(mut self) -> Result<(HashMap<String, String>, JsValue), JsValue> {
        while let Some(event) = self.next_event().await {
            match event {
                XhrEvent::Complete {
                    headers,
                    body
                } => return Ok((headers, body)),
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
    pub async fn response(&self) -> Result<Response, JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("No global window"))?;

        let resp_js = JsFuture::from(window.fetch_with_request(&self.request)).await?;
        resp_js.dyn_into::<Response>()
    }

    pub async fn text(&self) -> Result<String, JsValue> {
        let response = self.response().await?;
        let text_promise = response.text()?;
        let text_js = JsFuture::from(text_promise).await?;
        text_js.as_string()
            .ok_or_else(|| JsValue::from_str("Failed to convert response to string"))
    }

    pub async fn bytes(&self) -> Result<js_sys::Uint8Array, JsValue> {
        let response = self.response().await?;
        let array_buffer_promise = response.array_buffer()?;
        let array_buffer = JsFuture::from(array_buffer_promise).await?;
        Ok(js_sys::Uint8Array::new(&array_buffer))
    }

    pub async fn json<T: DeserializeOwned>(&self) -> Result<T, JsValue> {
        let text = self.text().await?;
        serde_json::from_str(&text)
            .map_err(|e| JsValue::from_str(&format!("JSON deserialization failed: {}", e)))
    }

    pub async fn json_raw(&self) -> Result<JsValue, JsValue> {
        let response = self.response().await?;
        let json_promise = response.json()?;
        JsFuture::from(json_promise).await
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