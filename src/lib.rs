//! traQ login is the [axum oauth example](https://github.com/tokio-rs/axum/tree/main/examples/oauth).
//! Notes are the [axum todos example](https://github.com/tokio-rs/axum/tree/main/examples/todos),
//! kept per logged-in user behind the repository and service from the
//! [hexagonal architecture guide](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust).

pub mod domain;
pub mod inbound;
pub mod outbound;
