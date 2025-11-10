// Sentium Bridge Protocol - Library
// Intent-based cross-chain communication with quantum-safe verification

pub mod core {
    pub mod router {
        include!("../core/router/mod.rs");
    }
    
    pub mod context {
        include!("../core/context/mod.rs");
    }
}

pub mod light_clients {
    include!("../light-clients/mod.rs");
}

// Re-exports for convenience
pub use core::router::{Router, Intent, IntentTranslator, ChainAdapter};
pub use core::context::{SemanticContext, ContextPreserver, UserPreferences};
pub use light_clients::{LightClient, LightClientManager, StateProof};
