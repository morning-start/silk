pub mod adapter;
pub mod adapters;
pub mod builtin_adapters;
pub mod registry;

pub use adapter::{ProtocolError, ProviderAdapter, UpstreamRequest, UpstreamResponse};
pub use registry::AdapterRegistry;
