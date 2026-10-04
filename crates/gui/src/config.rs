//! Persistent UI configuration. The format itself is shared with the
//! terminal frontend through [`appcore::config`], so both apps agree on where
//! `config.json` lives and what it contains.

pub use appcore::config::{AppConfig, get, save, update};