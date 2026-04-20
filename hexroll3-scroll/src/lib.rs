#[macro_use]
extern crate pest_derive;

pub mod commands;
pub mod frame;
pub mod generators;
pub mod instance;
pub mod parser;
pub mod renderer;
pub mod renderer_env;
pub mod repository;
pub mod semantics;

pub trait ValueUuidExt {
    fn uuid_as_str(&self) -> &str;
    fn uuid_as_value(&self) -> &Self;
}

impl ValueUuidExt for serde_json::Value {
    #[inline]
    fn uuid_as_str(&self) -> &str {
        self.as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap()
    }
    #[inline]
    fn uuid_as_value(&self) -> &Self {
        self.as_array().and_then(|a| a.first()).unwrap()
    }
}
