pub mod db;
pub mod export;
pub mod model;
pub mod profile;
pub mod repo;

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
