//! Hexagonal layout for yarimashow.
//!
//! - `domain` holds the business types, ports, and services.
//! - `inbound` adapts callers into the domain. HTTP is the only inbound adapter.
//! - `outbound` adapts the domain onto traQ, in-memory sessions, and DynamoDB.
//!
//! `main` only chooses adapters and starts the HTTP server.

pub mod config;
pub mod domain;
pub mod inbound;
pub mod outbound;
