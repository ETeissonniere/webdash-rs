#[cfg(feature = "ssr")]
mod page;

mod search;

use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Service {
    pub name: String,
    pub url: String,
    #[serde(default = "default_group")]
    pub group: String,
    #[serde(default)]
    pub description: String,
}

pub fn default_group() -> String {
    "Services".into()
}

impl Service {
    pub fn logo_path(&self) -> String {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        format!("/logo/{}", URL_SAFE_NO_PAD.encode(self.url.as_bytes()))
    }
}

#[cfg(feature = "ssr")]
pub use page::render_page;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_islands();
}
