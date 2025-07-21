use base64::engine::general_purpose;
use base64::Engine;
use chrono::Utc;
use futures::io::Cursor;
use futures::TryStreamExt;
use generic_array::GenericArray;
use mongodb::bson::doc;
use mongodb::bson::oid::{self, ObjectId};
use mongodb::{results, Collection};
use mothrbox_service_core::encryption_core::blocks::CipherBlock;
use openssl::derive;
use p256::ecdsa::signature::Keypair;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
// use aes_gcm::aes;
use crate::dto::key::KeyPairDTO;
use crate::dto::GenerateKeypairRequest;
use crate::middleware::api_token::AuthenticatedClient;
use crate::model_core::api_token::ApiToken;
use crate::model_core::key::KeyPair;
use crate::paste_id::PasteId;
use crate::piston;
use crate::piston::key::KeyService;
use crate::sui_core::sui_impl::{MtxType, SuiService};
use crate::sui_core::SuiCli;
use crate::walrus_core::walrus_impl;
use rocket::response::content::RawMsgPack;
use rocket::{
    data::ToByteUnit, fs::TempFile, http::uri::Absolute, response::stream::ByteStream,
    tokio::fs::File, Data,
};
use tokio::io::AsyncReadExt;
use tokio::stream;

use aes::Aes256;
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use rand::{rngs::OsRng, RngCore};
use std::fmt;
use std::{clone, io};

use rocket::data::Limits;

type Aes256Ctr = Ctr128BE<Aes256>;

#[get("/walrus_test")]
pub fn walrus_test() -> Result<String, rocket::response::status::Custom<String>> {
    let walrus_init = walrus_impl::WalrusCore {};

    match walrus_init.store(
        "/home/goldman/mothrbox/dummy.mp4",
        // "/home/goldman/mothrbox/client_config.yaml",
        // "/home/goldman/.sui/sui_config/client.yaml"
    ) {
        Ok(output) => Ok(output),
        Err(e) => Err(rocket::response::status::Custom(
            Status::InternalServerError,
            format!("Command failed: {}", e),
        )),
    }
}

#[post("/keys", data = "<key_pair>")]
pub async fn create_key(
    db: &State<Collection<KeyPair>>,
    key_pair: Json<GenerateKeypairRequest>,
) -> Result<Json<String>, rocket::response::status::Custom<String>> {
    let key_service = KeyService {};
    let res = key_service.create_key(db, key_pair).await;
    res
}

#[derive(rocket::form::FromForm)]
struct EncryptForm<'r> {
    owner: String,
    file: rocket::fs::TempFile<'r>,
}

#[post("/encrypt/<user_id>/<alias>", data = "<form>")] // 30 GB
pub async fn encrypt(
    form: rocket::form::Form<EncryptForm<'_>>,
    // authenticate user
    user_id: &str,
    alias: &str,
    db: &State<Collection<KeyPair>>,
    limits: &Limits,
) -> Option<serde_json::Value> {
    let encrypt_service = piston::EcryptionService {};
    let owner = form.owner.clone(); // extract the address

    let limit = limits.get("file").unwrap_or(1.gigabytes());

    // convert TempFile to Data<'_> or Vec<u8>
    let file_data = match form.file.open().await {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).await.ok()?;
            buffer
        }
        Err(_) => return None,
    };
    let encrypt_data = encrypt_service
        .encrypt(db, alias, user_id, owner.to_string(), file_data)
        .await;

    return encrypt_data;
}

#[get("/decrypt/<user_id>/<alias>/<blob_id>", /*data="<data>"*/)]
pub async fn decrypt(
    // data: Data<'_>,
    user_id: &str,
    alias: &str,
    blob_id: &str,
    db: &State<Collection<KeyPair>>,
) -> std::io::Result<RawMsgPack<Vec<u8>>> {
    let encrypt_service = piston::EcryptionService {};
    let decrypt_data = encrypt_service
        .fetch_and_decrypt(db, alias, user_id, blob_id)
        .await;

    return Ok(RawMsgPack(decrypt_data));
}

#[get("/create-kiosk")]
pub async fn create_kiosk_controller(
) -> Result<serde_json::Value, rocket::response::Debug<anyhow::Error>> {
    let result = SuiService::create_kiosk().await?;
    Ok(serde_json::to_value(result).unwrap())
}

#[derive(std::fmt::Debug, rocket::serde::Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct MintKiosk {
    pub alias: String,
    pub key_type: MtxType,
}

#[post("/mint-token-and-kiosk", data = "<request>")]
pub async fn mint_token_and_kiosk_controller(
    request: Json<MintKiosk>,
) -> Result<serde_json::Value, rocket::response::Debug<anyhow::Error>> {
    let result = SuiService::mint_token_and_kiosk(
        &request.alias,
        request.key_type.clone(), // Clone the enum if needed),
    )
    .await?;

    Ok(result)
}

// #[get("/keys")]
// pub async fn list_keys(
//     key_manager: &State<EccKeyManager>,
// ) -> Json<ApiResponse<KeyListResponse>> {
//     match key_manager.list_keys().await {
//         Ok(keys) => {
//             let total = keys.len();
//             Json(ApiResponse::success(KeyListResponse { keys, total }))
//         }
//         Err(e) => Json(ApiResponse::error(e.to_string())),
//     }
// }

// #[get("/keys/<key_id>/exists")]
// pub async fn key_exists(
//     key_manager: &State<EccKeyManager>,
//     key_id: &str,
// ) -> Json<ApiResponse<bool>> {
//     match key_manager.key_exists(key_id).await {
//         Ok(exists) => Json(ApiResponse::success(exists)),
//         Err(e) => Json(ApiResponse::error(e.to_string())),
//     }
// }

// #[post("/keys/sign", data = "<request>")]
// pub async fn sign_message(
//     key_manager: &State<EccKeyManager>,
//     request: Json<SignRequest>,
// ) -> Json<ApiResponse<String>> {
//     // Decode base64 message
//     let message = match base64::decode(&request.message) {
//         Ok(msg) => msg,
//         Err(e) => return Json(ApiResponse::error(format!("Invalid base64 message: {}", e))),
//     };

//     // Load key pair
//     let key_pair = match key_manager.load_key_pair(&request.key_id).await {
//         Ok(Some(kp)) => kp,
//         Ok(None) => return Json(ApiResponse::error("Key not found".to_string())),
//         Err(e) => return Json(ApiResponse::error(e.to_string())),
//     };

//     // Sign message
//     match key_pair.sign(&message) {
//         Ok(signature) => {
//             let signature_b64 = base64::encode(&signature);
//             Json(ApiResponse::success(signature_b64))
//         }
//         Err(e) => Json(ApiResponse::error(e.to_string())),
//     }
// }

// #[post("/keys/verify", data = "<request>")]
// pub async fn verify_signature(
//     key_manager: &State<EccKeyManager>,
//     request: Json<VerifyRequest>,
// ) -> Json<ApiResponse<bool>> {
//     // Decode base64 inputs
//     let message = match base64::decode(&request.message) {
//         Ok(msg) => msg,
//         Err(e) => return Json(ApiResponse::error(format!("Invalid base64 message: {}", e))),
//     };

//     let signature = match base64::decode(&request.signature) {
//         Ok(sig) => sig,
//         Err(e) => return Json(ApiResponse::error(format!("Invalid base64 signature: {}", e))),
//     };

//     // Load key pair
//     let key_pair = match key_manager.load_key_pair(&request.key_id).await {
//         Ok(Some(kp)) => kp,
//         Ok(None) => return Json(ApiResponse::error("Key not found".to_string())),
//         Err(e) => return Json(ApiResponse::error(e.to_string())),
//     };

//     // Verify signature
//     match key_pair.verify(&message, &signature) {
//         Ok(is_valid) => Json(ApiResponse::success(is_valid)),
//         Err(e) => Json(ApiResponse::error(e.to_string())),
//     }
// }

// #[delete("/keys/<key_id>")]
// pub async fn delete_key(
//     key_manager: &State<EccKeyManager>,
//     key_id: &str,
// ) -> Json<ApiResponse<bool>> {
//     match key_manager.delete_key_pair(key_id).await {
//         Ok(deleted) => Json(ApiResponse::success(deleted)),
//         Err(e) => Json(ApiResponse::error(e.to_string())),
//     }
// }

// #[post("/keys/<key_id>/deactivate")]
// pub async fn deactivate_key(
//     key_manager: &State<EccKeyManager>,
//     key_id: &str,
// ) -> Json<ApiResponse<bool>> {
//     match key_manager.deactivate_key_pair(key_id).await {
//         Ok(deactivated) => Json(ApiResponse::success(deactivated)),
//         Err(e) => Json(ApiResponse::error(e.to_string())),
//     }
// }

// #[get("/health")]
// pub fn health_check() -> Json<ApiResponse<String>> {
//     Json(ApiResponse::success("ECC Key Service is healthy".to_string()))
// }

#[post("/issue-token", data = "<wallet_address>")]
pub async fn issue_token(
    db: &State<Collection<ApiToken>>,
    wallet_address: Json<KeyPairDTO>,
) -> Result<String, Status> {
    let token = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let new_token = ApiToken {
        id: ObjectId::new(),
        token: token.clone(),
        allowed: true,
        owner: wallet_address.address,
        created_at: Some(now.into()),
        expires_at: None,
    };

    db.insert_one(&new_token, None)
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok(token)
}
