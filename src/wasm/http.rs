use std::collections::HashMap;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::{Blob, XmlHttpRequest as WasmXmlHttpRequest, Event, ProgressEvent, EventTarget, AbortController};
use js_sys::Uint8Array;
use futures::channel::mpsc::{unbounded, UnboundedReceiver};
use futures::channel::mpsc;
use futures::StreamExt;
use futures_util::lock::Mutex;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::utils::never_send::NeverSend;
use wasm_bindgen_futures::JsFuture;
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

#[derive(Clone)]
pub struct XmlHttpRequest {
    xml_http: Arc<NeverSend<WasmXmlHttpRequest>>,
    progress_rx: Arc<Mutex<UnboundedReceiver<ProgressEvent>>>,
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

    pub fn xhr(self) -> XmlHttpRequest {
        let url = self.url.expect("URL must be set before send()");
        let xhr = WasmXmlHttpRequest::new().unwrap();
        xhr.open(&self.method, &url).unwrap();

        for (k, v) in self.headers {
            xhr.set_request_header(&k, &v).unwrap();
        }

        let xml_http = Arc::new(NeverSend(xhr));
        let (tx, rx) = unbounded::<ProgressEvent>();

        {
            let tx_clone = tx.clone();
            let progress_closure = Closure::wrap(Box::new(move |e: ProgressEvent| {
                let _ = tx_clone.unbounded_send(e);
            }) as Box<dyn FnMut(_)>);

            xml_http.set_onprogress(Some(progress_closure.as_ref().unchecked_ref()));
        }

        match self.body {
            Some(Body::Text(s)) => xml_http.send_with_opt_str(Some(&s)).unwrap(),
            Some(Body::Blob(b)) => xml_http.send_with_opt_blob(Some(&b)).unwrap(),
            Some(Body::Bytes(ref bytes)) => {
                xml_http.send_with_opt_u8_array(Some(bytes)).unwrap();
            }
            Some(Body::Uint8Array(u8a)) => xml_http.send_with_opt_js_u8_array(Some(&u8a)).unwrap(),
            None => xml_http.send().unwrap(),
        };

        XmlHttpRequest {
            xml_http,
            progress_rx: Arc::new(Mutex::new(rx)),
        }
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

impl XmlHttpRequest {
    pub async fn response(&self) -> Result<JsValue, JsValue> {
        let xhr = &self.xml_http;

        // Either error or response come first
        let (tx, mut rx) = mpsc::channel::<Result<JsValue, JsValue>>(1);

        let onload = {
            let xhr = xhr.clone();
            let mut tx = tx.clone();
            Closure::once(Box::new(move |_: Event| {
                match xhr.response() {
                    Ok(resp) => {
                        let _ = tx.try_send(Ok(resp));
                    }
                    Err(e) => {
                        let _ = tx.try_send(Err(e));
                    }
                }
            }) as Box<dyn FnMut(_)>)
        };
        xhr.set_onload(Some(onload.as_ref().unchecked_ref()));

        let onerror = {
            let mut tx = tx.clone();
            Closure::once(Box::new(move |_: Event| {
                let _ = tx.try_send(Err(JsValue::from_str("Network error")));
            }) as Box<dyn FnMut(_)>)
        };
        xhr.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        let onabort = {
            let mut tx = tx.clone();
            Closure::once(Box::new(move |_: Event| {
                let _ = tx.try_send(Err(JsValue::from_str("Aborted")));
            }) as Box<dyn FnMut(_)>)
        };
        xhr.set_onabort(Some(onabort.as_ref().unchecked_ref()));

        let Some(result) = rx.next().await else {
            return Err(JsValue::from_str("Response channel closed"));
        };

        xhr.set_onload(None);
        xhr.set_onerror(None);
        xhr.set_onabort(None);
        xhr.set_onprogress(None);

        onload.forget();
        onerror.forget();
        onabort.forget();

        result
    }

    pub async fn next_event(&self) -> Option<ProgressEvent> {
        let mut rx = self.progress_rx.lock().await;
        rx.next().await
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