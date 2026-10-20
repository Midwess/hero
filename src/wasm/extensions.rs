//! Conversions between Rust byte containers and browser values.

use bytes::Bytes;
use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::JsValue;

/// Converts byte containers into JavaScript `Uint8Array` values.
pub trait VecExtension {
    /// Copies the bytes into a JavaScript-owned typed array.
    fn to_uint8_array(&self) -> Uint8Array;

    /// Copies the bytes into a JavaScript value containing a typed array.
    fn to_js_value(&self) -> JsValue {
        self.to_uint8_array().into()
    }

    /// Creates a JavaScript view into this value's current WASM memory.
    ///
    /// # Safety
    ///
    /// The caller must ensure the backing bytes stay alive and WASM memory is
    /// not grown while JavaScript uses the returned view.
    unsafe fn view_uint8_array(&self) -> Uint8Array;
}

/// Reads non-standard browser path metadata from uploaded files.
pub trait FileExtension {
    /// Returns Chromium's `webkitRelativePath` when a directory upload supplied it.
    fn webkit_path(&self) -> Option<String>;
}

impl FileExtension for web_sys::File {
    fn webkit_path(&self) -> Option<String> {
        let Ok(request_ctor) = Reflect::get(self, &JsValue::from_str("webkitRelativePath")) else {
            return None;
        };

        request_ctor.as_string()
    }
}

impl VecExtension for Vec<u8> {
    fn to_uint8_array(&self) -> Uint8Array {
        Uint8Array::from(self.as_slice())
    }

    unsafe fn view_uint8_array(&self) -> Uint8Array {
        unsafe { Uint8Array::view(self) }
    }
}

impl VecExtension for Bytes {
    fn to_uint8_array(&self) -> Uint8Array {
        Uint8Array::from(self.as_ref())
    }

    unsafe fn view_uint8_array(&self) -> Uint8Array {
        unsafe { Uint8Array::view(self) }
    }
}
