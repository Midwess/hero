#![cfg(all(target_family = "wasm", feature = "wasm"))]

use hero::wasm::HttpClient;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_dedicated_worker);

#[wasm_bindgen_test]
async fn fetch_and_consume_body_in_worker() {
    let mut response = HttpClient::new()
        .url("data:application/octet-stream,worker-response")
        .fetch()
        .unwrap()
        .send_streaming()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().unwrap().contains_key("content-type"));
    let mut bytes = Vec::new();
    while let Some(chunk) = response.next_chunk().await.unwrap() {
        bytes.extend(chunk);
    }
    assert_eq!(bytes, b"worker-response");
    assert!(response.next_chunk().await.unwrap().is_none());
}

#[wasm_bindgen_test]
async fn compatible_response_keeps_its_body_readable() {
    let response = HttpClient::new().url("data:text/plain,compatible-body").fetch().unwrap().send().await.unwrap();
    let body = wasm_bindgen_futures::JsFuture::from(response.text().unwrap()).await.unwrap();
    assert_eq!(body.as_string().unwrap(), "compatible-body");
}
