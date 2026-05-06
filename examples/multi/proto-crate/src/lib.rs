//! Generated ConnectRPC bindings for the multi-worker example.
//!
//! Both `echo-worker` and `gateway-worker` depend on this crate so that they
//! share a single source of truth for service definitions.
//!
//! Re-exports the generated package modules at the crate root, so callers
//! can write e.g. `multi_proto::echo::v1::EchoServiceExt`.

#[allow(warnings, unused)]
mod generated {
    // `connectrpc-build` unified mode emits `super::`-relative paths inside
    // each generated `pub mod`, so the include must live inside a module
    // (not at the crate root) to give `super::*` something to resolve to.
    connectrpc::include_generated!();
}

pub use generated::multi::*;
