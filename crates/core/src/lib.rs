pub mod chat;
pub mod ctx;
pub mod db;
pub mod embed;
pub mod export;
pub mod model;
pub mod profile;
pub mod repo;
pub mod report;
pub mod search;

pub mod context;

pub use ctx::Ctx;

/// 固定主题域（设计文档 §2.1）
pub const DOMAINS: &[&str] = &[
    "health",
    "wealth",
    "career",
    "family",
    "relationship",
    "preference",
    "goal",
];

pub const SENSITIVITIES: &[&str] = &["normal", "sensitive", "secret"];
