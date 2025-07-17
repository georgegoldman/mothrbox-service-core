use std::process::Command;

use anyhow::Ok;
use fastcrypto::hash::{Blake2b256, HashFunction, Sha3_256};
use sui_sdk::types::crypto::{Signer, SuiKeyPair, SuiSignature};

pub struct SuiCli;

impl SuiCli {
    pub fn command(&self) -> Command {
        let mut cmd = Command::new("sui");
        cmd
    }

    // pub fn get_active_wallet(&self) -> Result<String, std::io::Error> {
    //     let mut cmd = self.command();
    //     cmd.arg("client").arg("active-address").arg("--json");

    //     let output = cmd.output()?;

    //     if output.status.success() {
    //         Ok(String::from_utf8_lossy(&output.stdout).to_string())
    //     } else {
    //         Err(std::io::Error::new(
    //             std::io::ErrorKind::Other,
    //             format!(
    //                 "Command failed:\nStatus: {}\nStderr: {}",
    //                 output.status,
    //                 String::from_utf8_lossy(&output.stderr)
    //             ),
    //         ))
    //     }
    // }
}

pub struct SuiService;

impl SuiService {
    pub fn test_txn() {
        println!("Hi from the server");
    }

    pub async fn generate_key_nft() -> Result<serde_json::Value, anyhow::Error> {
        dotenv::dotenv().ok();
        let sui_client = sui_sdk::SuiClientBuilder::default()
            .build_testnet()
            .await
            .unwrap();
        let get_key_pair = sui_sdk::types::crypto::SuiKeyPair::decode(
            std::env::var("PRIVATE_KEY")
                .expect("err loading var")
                .as_str(),
        )
        .unwrap();

        let pk = get_key_pair.public();
        let sender = sui_sdk::types::base_types::SuiAddress::from(&pk);
        println!("Sender: {:?}", sender);

        // making sure the signer has enough gas
        let gas_coin = sui_client
            .coin_read_api()
            .get_coins(sender, None, None, None)
            .await
            .unwrap()
            .data
            .into_iter()
            .next()
            .ok_or(anyhow::anyhow!("No coin found for sender"))
            .unwrap();
        // constructing a programmable transaction
        let mut pt =
            sui_sdk::types::programmable_transaction_builder::ProgrammableTransactionBuilder::new();
        let package = sui_sdk::types::base_types::ObjectID::from_hex_literal(
            "0x7e59da94aa73e4c9b5c1aef34555ac97de1951254d779894e0a5824ccb4110c2",
        )
        .map_err(|err| anyhow::anyhow!(err))
        .unwrap();
        let module = sui_sdk::types::Identifier::new("key_validator")
            .map_err(|err| anyhow::anyhow!(err))
            .unwrap();
        let function = sui_sdk::types::Identifier::new("create_kiosk")
            .map_err(|err| anyhow::anyhow!(err))
            .unwrap();
        pt.move_call(package, module, function, vec![], vec![]);
        let builder = pt.finish();
        let gas_budget = 10_000_000;
        let gas_price = sui_client
            .read_api()
            .get_reference_gas_price()
            .await
            .unwrap();
        let tx_data = sui_sdk::types::transaction::TransactionData::new_programmable(
            sender,
            vec![gas_coin.object_ref()],
            builder,
            gas_budget,
            gas_price,
        );

        let intent_msg = shared_crypto::intent::IntentMessage::new(
            shared_crypto::intent::Intent::sui_transaction(),
            tx_data,
        );
        let raw_tx = bcs::to_bytes(&intent_msg).expect("bcs should not fail");
        let mut hasher = Blake2b256::default();
        hasher.update(&raw_tx);
        let digest_bytes = hasher.finalize();

        let digest = sui_sdk::types::base_types::TransactionDigest::new(digest_bytes.digest);
        // create an intent message
        if let sui_sdk::types::crypto::SuiKeyPair::Ed25519(kp) = &get_key_pair {
            let sui_sig: sui_sdk::types::crypto::Signature = kp.sign(&digest.into_inner());
            let res = sui_sig.verify_secure(
                &intent_msg,
                sender,
                sui_sdk::types::crypto::SignatureScheme::ED25519,
            );
            assert!(res.is_ok());

            // execute transaction
            let transaction_response = sui_client
                .quorum_driver_api()
                .execute_transaction_block(
                    sui_sdk::types::transaction::Transaction::from_generic_sig_data(
                        intent_msg.value,
                        vec![sui_sdk::types::signature::GenericSignature::Signature(
                            sui_sig,
                        )],
                    ),
                    sui_sdk::rpc_types::SuiTransactionBlockResponseOptions::new()
                        .with_input()
                        .with_raw_input()
                        .with_effects()
                        .with_events()
                        .with_object_changes()
                        .with_balance_changes()
                        .with_raw_effects()
                        .with_balance_changes(),
                    None,
                )
                .await?;
            return Ok(serde_json::to_value(transaction_response)?);
        } else {
            anyhow::bail!("Expected Ed25519 keypair")
        }

        // make an in memory store

        // let tx =
        //     sui_sdk::types::transaction::Transaction::
        // // now sign the transaction
        // let signature = sui_client.quorum_driver_api().execute_transaction_block(
        //     tx_data,
        //     sui_sdk::rpc_types::SuiTransactionBlockResponseOptions,
        //     request_type,
        // );
    }
}
