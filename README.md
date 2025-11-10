# Sentium Bridge Protocol (SBP)

**Semantic Bridge Protocol - Intent-Based Cross-Chain Communication**

## Overview

Sentium Bridge Protocol (SBP) is a standalone cross-chain communication protocol that enables intent-based messaging between different blockchains. Unlike traditional packet-based protocols (like Cosmos IBC), SBP preserves semantic context and uses AI-powered routing for optimal cross-chain transactions.

## Why Separate Repository?

- ✅ Different chains can use SBP independently (not just Sentium chains)
- ✅ Independent versioning and releases
- ✅ Multiple language implementations (Rust, Go, Python)
- ✅ Focused security audits
- ✅ Bridge developers can contribute without touching core SDK

## Features

- **Intent-Based Messaging**: Preserve semantic context across chains
- **AI-Powered Routing**: Optimal path finding using graph neural networks
- **Quantum-Safe Light Clients**: Dilithium5-based state verification
- **Multi-Chain Support**: Ethereum, Bitcoin, Polkadot, Cosmos, and more
- **Context Preservation**: Maintain intent context across chain boundaries

## Architecture

```
sentium-bridge/
├── core/           # Core bridge logic (Rust)
├── light-clients/  # Quantum-safe light clients (Rust + Go)
├── router/         # AI-powered routing (Python + Rust)
├── adapters/       # Chain-specific adapters (Rust + Go)
└── relayer/        # Message relay infrastructure (Go)
```

## Comparison: IBC vs SBP

| Feature | Cosmos IBC | Sentium SBP |
|---------|-----------|-------------|
| Messaging | Packet-based | Intent-based |
| Routing | Static | AI-optimized |
| Security | Classical crypto | Quantum-resistant |
| Context | None | Preserved |
| Language | Go | Rust + Go + Python |

## Installation

```bash
# Rust implementation
cargo add sentium-bridge

# Go implementation
go get github.com/sentium/sentium-bridge-go

# Python AI router
pip install sentium-bridge-py
```

## Quick Start

```rust
use sentium_bridge::{Bridge, Intent, Chain};

// Create bridge instance
let bridge = Bridge::new()
    .add_chain(Chain::Ethereum)
    .add_chain(Chain::Polkadot)
    .build();

// Send cross-chain intent
let intent = Intent::new()
    .from_chain(Chain::Ethereum)
    .to_chain(Chain::Polkadot)
    .action("transfer")
    .amount(100)
    .build();

bridge.send(intent).await?;
```

## Documentation

- [Architecture](docs/architecture.md)
- [API Reference](docs/api.md)
- [Integration Guide](docs/integration.md)
- [Security Model](docs/security.md)

---

**© 2025 Sentium Foundation. All rights reserved.**

*Building the quantum-resistant future of blockchain.*
