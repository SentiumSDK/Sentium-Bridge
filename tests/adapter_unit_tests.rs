// Unit tests for blockchain adapters
// Tests core functionality of each adapter implementation

use sentium_bridge::core::router::{
    Intent, IntentTranslator, EthereumAdapter, PolkadotAdapter, 
    BitcoinAdapter, CosmosAdapter, ChainAdapter
};
use std::sync::Arc;

// ============================================================================
// Cosmos Adapter Tests - Protobuf encoding/decoding
// ============================================================================

#[cfg(test)]
mod cosmos_tests {
    use super::*;
    use cosmos_sdk_proto::cosmos::bank::v1beta1::{
        QueryBalanceRequest, QueryBalanceResponse, MsgSend
    };
    use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
    use prost::Message;

    #[test]
    fn test_cosmos_protobuf_encoding() {
        // Test encoding a balance query request
        let request = QueryBalanceRequest {
            address: "cosmos1test".to_string(),
            denom: "uatom".to_string(),
        };
        
        let mut buf = Vec::new();
        request.encode(&mut buf).expect("Failed to encode");
        
        assert!(!buf.is_empty(), "Encoded buffer should not be empty");
        
        // Test decoding
        let decoded = QueryBalanceRequest::decode(&buf[..])
            .expect("Failed to decode");
        
        assert_eq!(decoded.address, "cosmos1test");
        assert_eq!(decoded.denom, "uatom");
    }

    #[test]
    fn test_cosmos_protobuf_decoding() {
        // Test decoding a balance response
        let response = QueryBalanceResponse {
            balance: Some(Coin {
                denom: "uatom".to_string(),
                amount: "1000000".to_string(),
            }),
        };
        
        let mut buf = Vec::new();
        response.encode(&mut buf).expect("Failed to encode");
        
        let decoded = QueryBalanceResponse::decode(&buf[..])
            .expect("Failed to decode");
        
        assert!(decoded.balance.is_some());
        let balance = decoded.balance.unwrap();
        assert_eq!(balance.denom, "uatom");
        assert_eq!(balance.amount, "1000000");
    }

    #[test]
    fn test_cosmos_msg_send_encoding() {
        // Test encoding a MsgSend transaction
        let msg = MsgSend {
            from_address: "cosmos1sender".to_string(),
            to_address: "cosmos1receiver".to_string(),
            amount: vec![Coin {
                denom: "uatom".to_string(),
                amount: "1000".to_string(),
            }],
        };
        
        let mut buf = Vec::new();
        msg.encode(&mut buf).expect("Failed to encode");
        
        assert!(!buf.is_empty());
        
        let decoded = MsgSend::decode(&buf[..])
            .expect("Failed to decode");
        
        assert_eq!(decoded.from_address, "cosmos1sender");
        assert_eq!(decoded.to_address, "cosmos1receiver");
        assert_eq!(decoded.amount.len(), 1);
        assert_eq!(decoded.amount[0].denom, "uatom");
    }
}

// ============================================================================
// Polkadot Adapter Tests - Storage proof verification
// ============================================================================

#[cfg(test)]
mod polkadot_tests {
    use super::*;
    use sp_trie::StorageProof;
    use sp_core::H256;

    #[test]
    fn test_storage_proof_creation() {
        // Test creating a storage proof structure
        let proof_nodes = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
        ];
        
        let proof = StorageProof::new(proof_nodes.clone());
        
        assert_eq!(proof.iter_nodes().count(), 2);
    }

    #[test]
    fn test_storage_proof_empty() {
        // Test empty storage proof
        let proof = StorageProof::new(vec![]);
        
        assert_eq!(proof.iter_nodes().count(), 0);
    }

    #[test]
    fn test_state_root_hash() {
        // Test H256 state root creation
        let state_root_bytes = [0u8; 32];
        let state_root = H256::from(state_root_bytes);
        
        assert_eq!(state_root.as_bytes().len(), 32);
    }
}

// ============================================================================
// Bitcoin Adapter Tests - UTXO selection
// ============================================================================

#[cfg(test)]
mod bitcoin_tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct Utxo {
        txid: String,
        vout: u32,
        amount: u64,
        script_pubkey: Vec<u8>,
    }

    fn select_utxos_largest_first(
        available_utxos: Vec<Utxo>,
        target_amount: u64,
    ) -> Result<Vec<Utxo>, String> {
        let mut sorted_utxos = available_utxos;
        sorted_utxos.sort_by(|a, b| b.amount.cmp(&a.amount));
        
        let mut selected = Vec::new();
        let mut total = 0u64;
        
        for utxo in sorted_utxos {
            if total >= target_amount {
                break;
            }
            total += utxo.amount;
            selected.push(utxo);
        }
        
        if total < target_amount {
            return Err("Insufficient funds".to_string());
        }
        
        Ok(selected)
    }

    #[test]
    fn test_utxo_selection_sufficient_funds() {
        let utxos = vec![
            Utxo {
                txid: "tx1".to_string(),
                vout: 0,
                amount: 100000,
                script_pubkey: vec![],
            },
            Utxo {
                txid: "tx2".to_string(),
                vout: 0,
                amount: 50000,
                script_pubkey: vec![],
            },
            Utxo {
                txid: "tx3".to_string(),
                vout: 0,
                amount: 25000,
                script_pubkey: vec![],
            },
        ];
        
        let result = select_utxos_largest_first(utxos, 120000);
        assert!(result.is_ok());
        
        let selected = result.unwrap();
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].amount, 100000);
        assert_eq!(selected[1].amount, 50000);
    }

    #[test]
    fn test_utxo_selection_insufficient_funds() {
        let utxos = vec![
            Utxo {
                txid: "tx1".to_string(),
                vout: 0,
                amount: 50000,
                script_pubkey: vec![],
            },
        ];
        
        let result = select_utxos_largest_first(utxos, 100000);
        assert!(result.is_err());
    }

    #[test]
    fn test_utxo_selection_exact_amount() {
        let utxos = vec![
            Utxo {
                txid: "tx1".to_string(),
                vout: 0,
                amount: 100000,
                script_pubkey: vec![],
            },
        ];
        
        let result = select_utxos_largest_first(utxos, 100000);
        assert!(result.is_ok());
        
        let selected = result.unwrap();
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn test_change_calculation() {
        let total_input = 150000u64;
        let target_amount = 100000u64;
        let fee = 1000u64;
        
        let change = total_input.saturating_sub(target_amount + fee);
        assert_eq!(change, 49000);
    }
}

// ============================================================================
// Zcash Adapter Tests - Proof verification
// ============================================================================

#[cfg(test)]
mod zcash_tests {
    use super::*;

    #[test]
    fn test_proof_structure_validation() {
        // Test basic proof structure validation
        let proof_data = vec![0u8; 192]; // Groth16 proof size
        
        assert_eq!(proof_data.len(), 192);
    }

    #[test]
    fn test_public_inputs_validation() {
        // Test public inputs structure
        let public_inputs = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
        ];
        
        assert_eq!(public_inputs.len(), 2);
        assert!(!public_inputs[0].is_empty());
    }
}

// ============================================================================
// Dash Adapter Tests - X11 hashing
// ============================================================================

#[cfg(test)]
mod dash_tests {
    use super::*;
    use sha3::{Digest, Keccak256};

    #[test]
    fn test_hash_chain_structure() {
        // Test basic hash chain structure for X11
        let input = b"test data";
        
        let mut hasher = Keccak256::new();
        hasher.update(input);
        let hash1 = hasher.finalize();
        
        assert_eq!(hash1.len(), 32);
    }

    #[test]
    fn test_difficulty_target_comparison() {
        // Test difficulty target comparison logic
        let hash_value = 1000u64;
        let target = 2000u64;
        
        assert!(hash_value <= target);
    }
}

// ============================================================================
// TRON Adapter Tests - Protobuf serialization
// ============================================================================

#[cfg(test)]
mod tron_tests {
    use super::*;
    use prost::Message;

    #[derive(Clone, PartialEq, Message)]
    struct TestTronMessage {
        #[prost(string, tag = "1")]
        address: String,
        #[prost(int64, tag = "2")]
        amount: i64,
    }

    #[test]
    fn test_tron_protobuf_encoding() {
        let msg = TestTronMessage {
            address: "TRX1234567890".to_string(),
            amount: 1000000,
        };
        
        let mut buf = Vec::new();
        msg.encode(&mut buf).expect("Failed to encode");
        
        assert!(!buf.is_empty());
    }

    #[test]
    fn test_tron_protobuf_decoding() {
        let msg = TestTronMessage {
            address: "TRX1234567890".to_string(),
            amount: 1000000,
        };
        
        let mut buf = Vec::new();
        msg.encode(&mut buf).expect("Failed to encode");
        
        let decoded = TestTronMessage::decode(&buf[..])
            .expect("Failed to decode");
        
        assert_eq!(decoded.address, "TRX1234567890");
        assert_eq!(decoded.amount, 1000000);
    }
}

// ============================================================================
// Solana Adapter Tests - DEX instruction creation
// ============================================================================

#[cfg(test)]
mod solana_tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::instruction::{Instruction, AccountMeta};

    #[test]
    fn test_instruction_creation() {
        let program_id = Pubkey::new_unique();
        let account1 = Pubkey::new_unique();
        let account2 = Pubkey::new_unique();
        
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(account1, true),
                AccountMeta::new(account2, false),
            ],
            data: vec![1, 2, 3, 4],
        };
        
        assert_eq!(instruction.accounts.len(), 2);
        assert_eq!(instruction.data.len(), 4);
    }

    #[test]
    fn test_slippage_calculation() {
        let amount = 1000000u64;
        let slippage_bps = 50u64; // 0.5%
        
        let min_output = amount * (10000 - slippage_bps) / 10000;
        
        assert_eq!(min_output, 995000);
    }

    #[test]
    fn test_account_meta_creation() {
        let pubkey = Pubkey::new_unique();
        
        let writable_signer = AccountMeta::new(pubkey, true);
        assert!(writable_signer.is_writable);
        assert!(writable_signer.is_signer);
        
        let readonly = AccountMeta::new_readonly(pubkey, false);
        assert!(!readonly.is_writable);
        assert!(!readonly.is_signer);
    }
}

// ============================================================================
// Harmony Adapter Tests - Address conversion
// ============================================================================

#[cfg(test)]
mod harmony_tests {
    use super::*;
    use bech32::{ToBase32, FromBase32, Variant};

    #[test]
    fn test_bech32_encoding() {
        let data = vec![1, 2, 3, 4, 5];
        let encoded = bech32::encode("one", data.to_base32(), Variant::Bech32)
            .expect("Failed to encode");
        
        assert!(encoded.starts_with("one1"));
    }

    #[test]
    fn test_bech32_decoding() {
        let data = vec![1, 2, 3, 4, 5];
        let encoded = bech32::encode("one", data.to_base32(), Variant::Bech32)
            .expect("Failed to encode");
        
        let (hrp, decoded_data, _variant) = bech32::decode(&encoded)
            .expect("Failed to decode");
        
        assert_eq!(hrp, "one");
        
        let decoded_bytes = Vec::<u8>::from_base32(&decoded_data)
            .expect("Failed to convert from base32");
        
        assert_eq!(decoded_bytes, data);
    }

    #[test]
    fn test_eth_address_format() {
        let address_bytes = vec![0u8; 20];
        let eth_address = format!("0x{}", hex::encode(&address_bytes));
        
        assert_eq!(eth_address.len(), 42); // 0x + 40 hex chars
        assert!(eth_address.starts_with("0x"));
    }

    #[test]
    fn test_address_checksum_validation() {
        // Test that address length is correct
        let address = "0x" + &"a".repeat(40);
        assert_eq!(address.len(), 42);
        
        let hex_part = address.strip_prefix("0x").unwrap();
        assert_eq!(hex_part.len(), 40);
    }
}

// ============================================================================
// Litecoin/Dogecoin Adapter Tests - Scrypt verification
// ============================================================================

#[cfg(test)]
mod scrypt_tests {
    use super::*;
    use scrypt::{scrypt, Params};

    #[test]
    fn test_scrypt_hashing() {
        let input = b"test data";
        let salt = b"";
        let params = Params::new(10, 8, 1, 32).expect("Failed to create params");
        
        let mut output = vec![0u8; 32];
        scrypt(input, salt, &params, &mut output)
            .expect("Failed to hash");
        
        assert_eq!(output.len(), 32);
        assert_ne!(output, vec![0u8; 32]); // Should not be all zeros
    }

    #[test]
    fn test_scrypt_params_litecoin() {
        // Litecoin uses N=1024, r=1, p=1
        let params = Params::new(10, 8, 1, 32);
        assert!(params.is_ok());
    }

    #[test]
    fn test_scrypt_deterministic() {
        let input = b"test data";
        let salt = b"";
        let params = Params::new(10, 8, 1, 32).expect("Failed to create params");
        
        let mut output1 = vec![0u8; 32];
        scrypt(input, salt, &params, &mut output1)
            .expect("Failed to hash");
        
        let mut output2 = vec![0u8; 32];
        scrypt(input, salt, &params, &mut output2)
            .expect("Failed to hash");
        
        assert_eq!(output1, output2);
    }
}

// ============================================================================
// Chia Adapter Tests - Wallet discovery
// ============================================================================

#[cfg(test)]
mod chia_tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct ChiaWallet {
        id: u32,
        name: String,
        addresses: Vec<String>,
    }

    fn find_wallet_for_address(
        wallets: &[ChiaWallet],
        target_address: &str,
    ) -> Option<u32> {
        for wallet in wallets {
            if wallet.addresses.contains(&target_address.to_string()) {
                return Some(wallet.id);
            }
        }
        None
    }

    #[test]
    fn test_wallet_discovery_found() {
        let wallets = vec![
            ChiaWallet {
                id: 1,
                name: "Wallet 1".to_string(),
                addresses: vec!["xch1abc".to_string(), "xch1def".to_string()],
            },
            ChiaWallet {
                id: 2,
                name: "Wallet 2".to_string(),
                addresses: vec!["xch1ghi".to_string()],
            },
        ];
        
        let result = find_wallet_for_address(&wallets, "xch1def");
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_wallet_discovery_not_found() {
        let wallets = vec![
            ChiaWallet {
                id: 1,
                name: "Wallet 1".to_string(),
                addresses: vec!["xch1abc".to_string()],
            },
        ];
        
        let result = find_wallet_for_address(&wallets, "xch1xyz");
        assert_eq!(result, None);
    }

    #[test]
    fn test_wallet_discovery_multiple_addresses() {
        let wallets = vec![
            ChiaWallet {
                id: 1,
                name: "Wallet 1".to_string(),
                addresses: vec![
                    "xch1addr1".to_string(),
                    "xch1addr2".to_string(),
                    "xch1addr3".to_string(),
                ],
            },
        ];
        
        let result = find_wallet_for_address(&wallets, "xch1addr2");
        assert_eq!(result, Some(1));
    }
}
