use std::{process::Command, str::FromStr};

use anyhow::Ok;
use fastcrypto::hash::{Blake2b256, HashFunction, Sha3_256};
use move_core_types::parsing::types;
use rocket::form;
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

#[derive(std::fmt::Debug, Clone, rocket::serde::Deserialize, rocket::serde::Serialize)]
pub enum MtxType {
    MTXAccess,
    MTXKey,
}

pub struct SuiService;

impl SuiService {
    pub fn test_txn() {
        println!("Hi from the server");
    }

    pub async fn create_kiosk() -> Result<serde_json::Value, anyhow::Error> {
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

    pub async fn mint_token_and_kiosk(
        value: &str,
        description: &str,
        key_type: MtxType,
    ) -> Result<serde_json::Value, anyhow::Error> {
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

        let coin = sui_client
            .coin_read_api()
            .get_coins(sender, None, None, None)
            .await
            .unwrap()
            .data
            .into_iter()
            .next()
            .ok_or(anyhow::anyhow!("no coin found for sender"))
            .unwrap();

        // constructing a programmable txn
        let mut ptb =
            sui_sdk::types::programmable_transaction_builder::ProgrammableTransactionBuilder::new();
        let package = sui_sdk::types::base_types::ObjectID::from_hex_literal(
            "0x7e59da94aa73e4c9b5c1aef34555ac97de1951254d779894e0a5824ccb4110c2",
        )
        .map_err(|err| anyhow::anyhow!(err))
        .unwrap();
        let module = sui_sdk::types::Identifier::new("key_validator")
            .map_err(|err| anyhow::anyhow!(err))
            .unwrap();
        let function = sui_sdk::types::Identifier::new("mint_token_and_kiosk")
            .map_err(|err| anyhow::anyhow!(err))
            .unwrap();
        let mut type_arguments = vec![];
        match key_type {
            MtxType::MTXAccess => {
                type_arguments[0] = sui_sdk::types::TypeTag::from_str("0x7e59da94aa73e4c9b5c1aef34555ac97de1951254d779894e0a5824ccb4110c2::key_validator::MTXAccess").unwrap()
            }
            MtxType::MTXKey => {
                type_arguments[0] = sui_sdk::types::TypeTag::from_str("0x7e59da94aa73e4c9b5c1aef34555ac97de1951254d779894e0a5824ccb4110c2::key_validator::MTXKey").unwrap();
            }
        };

        let kiosk_arg = ptb
            .input(sui_sdk::types::transaction::CallArg::Object(
                /* convert the id to object arg */
                sui_sdk::types::transaction::ObjectArg::ImmOrOwnedObject((
                    sui_sdk::types::base_types::ObjectID::from_hex_literal(
                        "0x35879a101f643ef93e77e135fc7c63598a4527399680ba35e8c8146f89b347c9",
                    )
                    .unwrap(),
                    sui_sdk::types::base_types::SequenceNumber::from_u64(497848450_u64),
                    sui_sdk::types::base_types::ObjectDigest::from_str(
                        "7oNYTS92e3MujUxZCDMsP1cLFeWmf8ctoWz46fDkBUs4",
                    )
                    .unwrap(),
                )),
            ))
            .unwrap();
        let cap_arg = ptb
            .input(sui_sdk::types::transaction::CallArg::Object(
                /* convert the id to object arg */
                sui_sdk::types::transaction::ObjectArg::ImmOrOwnedObject((
                    sui_sdk::types::base_types::ObjectID::from_hex_literal(
                        "0x40a81c25e1c5f7d7d795ebc950678beb48c8bef170bbc85ce66671f97e7cfc7c",
                    )
                    .unwrap(),
                    sui_sdk::types::base_types::SequenceNumber::from_u64(497848450_u64),
                    sui_sdk::types::base_types::ObjectDigest::from_str(
                        "DiHUx3Dgwmpv1jSVNJVJteXzhsDDEZD6b95r4TBpMCij",
                    )
                    .unwrap(),
                )),
            ))
            .unwrap();
        let arguments = vec![
            ptb.input(sui_sdk::types::transaction::CallArg::Pure(
                bcs::to_bytes(&name).unwrap(),
            ))
            .unwrap(),
            ptb.input(sui_sdk::types::transaction::CallArg::Pure(
                bcs::to_bytes(&image_url).unwrap(),
            ))
            .unwrap(),
            ptb.input(sui_sdk::types::transaction::CallArg::Pure(
                bcs::to_bytes(&description).unwrap(),
            ))
            .unwrap(),
            kiosk_arg,
            cap_arg,
        ];
        let gas_budget = 10_000_000;
        let gas_price = sui_client
            .read_api()
            .get_reference_gas_price()
            .await
            .unwrap();

        ptb.command(sui_sdk::types::transaction::Command::move_call(
            package,
            module,
            function,
            type_arguments,
            arguments,
        ));

        let tx_data = sui_sdk::types::transaction::TransactionData::new_programmable(
            sender,
            vec![coin.object_ref()],
            ptb.finish(),
            gas_budget,
            gas_price,
        );

        let intent_msg = shared_crypto::intent::IntentMessage::new(
            shared_crypto::intent::Intent::sui_transaction(),
            tx_data,
        );

        let raw_tx = bcs::to_bytes(&intent_msg).unwrap();
        let mut hasher = Blake2b256::default();
        hasher.update(&raw_tx);
        let digest_byte = hasher.finalize();

        let digest = sui_sdk::types::base_types::TransactionDigest::new(digest_byte.digest);
        // create an intent
        if let sui_sdk::types::crypto::SuiKeyPair::Ed25519(kp) = &get_key_pair {
            let sui_sig: sui_sdk::types::crypto::Signature = kp.sign(&digest.into_inner());
            let res = sui_sig.verify_secure(
                &intent_msg,
                sender,
                sui_sdk::types::crypto::SignatureScheme::ED25519,
            );
            assert!(res.is_ok());

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
                        .with_balance_changes()
                        .with_effects()
                        .with_events()
                        .with_input()
                        .with_object_changes()
                        .with_raw_effects()
                        .with_raw_input(),
                    None,
                )
                .await?;
            return Ok(serde_json::to_value(transaction_response)?);
        } else {
            anyhow::bail!("Expected Ed25519 keypair")
        }
    }
}
