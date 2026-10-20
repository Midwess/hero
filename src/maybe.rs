//! Target-dependent marker bounds for APIs shared by native and WebAssembly.
//!
//! Native builds require the corresponding `Send`/`Sync` auto traits. Browser
//! builds accept every type, allowing the same generic API to run on a
//! single-threaded executor without conditional bounds at each call site.

#[cfg(target_family = "wasm")]
/// Requires `Send + Sync` on native targets and has no extra bound on WASM.
pub trait MaybeSendSync {}
#[cfg(not(target_family = "wasm"))]
/// Requires `Send + Sync` on native targets and has no extra bound on WASM.
pub trait MaybeSendSync: Sync + Send {}

#[cfg(target_family = "wasm")]
impl<T> MaybeSendSync for T {}

#[cfg(not(target_family = "wasm"))]
impl<T> MaybeSendSync for T where T: Sync + Send {}

#[cfg(not(target_family = "wasm"))]
/// Requires `Send` on native targets and has no extra bound on WASM.
pub trait MaybeSend: Send {}

#[cfg(not(target_family = "wasm"))]
impl<T> MaybeSend for T where T: Send {}

#[cfg(target_family = "wasm")]
/// Requires `Send` on native targets and has no extra bound on WASM.
pub trait MaybeSend {}

#[cfg(target_family = "wasm")]
impl<T> MaybeSend for T {}
