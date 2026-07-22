//! In-app browser: pure tab/history model, download filing rules, unzip
//! safety, and the webview host glue. Tabs are app orchestration, not media
//! math — nothing here belongs in the engine.

pub mod downloads;
pub mod tabs;
