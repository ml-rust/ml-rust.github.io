//! The browser search module of the catalogue site.
//!
//! The wasm32 build re-exports every `#[wasm_bindgen]` function of
//! `oxidoc_search::wasm`. The re-export makes this cdylib link the engine,
//! so the exports reach the `.wasm` file. `wasm-bindgen --target web` then
//! writes `oxidoc_search.js` and `oxidoc_search_bg.wasm` for `/search/`.
//! `scripts/ci/build_search_wasm.sh` runs both steps.
//!
//! A native build holds no code.

#[cfg(target_arch = "wasm32")]
pub use oxidoc_search::wasm::*;
