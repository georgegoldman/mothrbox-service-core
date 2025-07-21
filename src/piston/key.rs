use std::{fmt::format, io::Write};

use crate::{encryption_core::openssl_ecc_key_gen::OpensslEccKeyGen, walrus_core};
use futures::stream;
use mongodb::{
    bson::oid::{self, ObjectId},
    Collection,
};
use rand::Rng;
use rocket::{data::ToByteUnit, response, serde::json::Json, State};
use tokio::io::AsyncReadExt;

use crate::{dto::GenerateKeypairRequest, model_core::key::KeyPair};
use aes::Aes128;
use block_modes::{block_padding::Pkcs7, BlockMode, Cbc};

type Aes128Cbc = Cbc<Aes128, Pkcs7>;

pub struct KeyService;

impl KeyService {
    pub async fn create_key(
        &self,
        db: &State<Collection<KeyPair>>,
        key_pair: Json<GenerateKeypairRequest>,
    ) -> Result<Json<String>, rocket::response::status::Custom<String>> {
        let filter = mongodb::bson::doc! {"alias": &key_pair.alias};

        if let Ok(Some(_)) = db.find_one(filter.clone(), None).await {
            return Err(rocket::response::status::Custom(
                rocket::http::Status::Conflict,
                "Alias exist".to_string(),
            ));
        }

        let now = mongodb::bson::DateTime::now();
        let key_engine = OpensslEccKeyGen {};
        let value = key_engine.get_key();

        let owner_id = match ObjectId::parse_str(key_pair.owner.clone()) {
            Ok(oid) => oid,
            Err(e) => {
                return Err(rocket::response::status::Custom(
                    rocket::http::Status::InternalServerError,
                    format!("Database error: {}", e),
                ))
            }
        };

        let new_key = KeyPair {
            id: None,
            owner: owner_id,
            algorithm: Some(key_pair.algorithm.clone()),
            alias: key_pair.alias.clone(),
            value: value.to_vec(),
            is_active: true,
            created_at: Some(now),
            updated_at: Some(now),
        };

        let result = db.insert_one(new_key, None).await;

        match result {
            Ok(_) => Ok(Json("key pair read in to storage".to_string())),
            Err(e) => Err(rocket::response::status::Custom(
                rocket::http::Status::BadRequest,
                format!("Error: {e}"),
            )),
        }
    }
}

pub struct EcryptionService;

impl EcryptionService {
    pub async fn decrypt(
        &self,
        db: &State<Collection<KeyPair>>,
        alias: &str,
        user_id: &str,
        encrypted_data: Vec<u8>, // data: rocket::Data<'_>
    ) -> Vec<u8> {
        let mut buffer = encrypted_data;
        // let mut stream = data.open(5.megabytes());
        // stream.read_to_end(&mut buffer).await;

        let user = match ObjectId::parse_str(user_id.clone()) {
            Ok(oid) => oid,
            Err(e) => {
                eprintln!("Invalid user id: {}", e);
                return b"invalid user id".to_vec();
            }
        };

        let filter = mongodb::bson::doc! {"alias": alias};

        let keypair = match db.find_one(filter, None).await {
            Ok(Some(keypair)) => keypair,
            Ok(None) => return b"this is data doesn't exist on the db".to_vec(),
            Err(e) => {
                return b"there is an issue trying to run some operation on the database".to_vec()
            }
        };

        let openssl_instance = OpensslEccKeyGen {};
        let iv = openssl_instance.get_iv();

        let cipher: Cbc<Aes128, Pkcs7> = Aes128Cbc::new_from_slices(&keypair.value, &iv).unwrap();

        let decrypted_text = cipher.decrypt_vec(&buffer).unwrap();

        decrypted_text
    }

    pub async fn fetch_and_decrypt(
        &self,
        db: &State<Collection<KeyPair>>,
        alias: &str,
        user_id: &str,
        blob_id: &str,
    ) -> Vec<u8> {
        // fetch the blob from walrus
        let fetch_blob = walrus_core::WalrusCore {}.read(blob_id);

        let encrypted_data = match fetch_blob {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => {
                eprintln!("Failed to read response bytes: {}", e);
                return b"ailed to read blob data".to_vec();
            }
        };

        self.decrypt(db, alias, user_id, encrypted_data).await
    }

    pub async fn encrypt(
        &self,
        db: &State<Collection<KeyPair>>,
        alias: &str,
        user_id: &str,
        owner: String,
        data: Vec<u8>,
    ) -> Option<serde_json::Value> {
        // let _ = str;
        let mut buffer = data;
        //  let mut stream = data.open(5.megabytes());
        //  stream.read_to_end(&mut buffer).await;

        let user = match ObjectId::parse_str(user_id.clone()) {
            Ok(oid) => oid,
            Err(e) => {
                eprintln!("Invalid user id: {}", e);
                return None;
            }
        };
        let filter = mongodb::bson::doc! {"alias": alias};

        let keypair = match db.find_one(filter, None).await {
            Ok(Some(keypair)) => keypair,
            Ok(None) => {
                eprintln!("Key not found in DB");
                return None;
            }
            Err(e) => {
                eprintln!("DB error: {}", e);
                return None;
            }
        };

        let openssl_instance = OpensslEccKeyGen {};
        let iv = openssl_instance.get_iv();

        let cipher: Cbc<Aes128, Pkcs7> = Aes128Cbc::new_from_slices(&keypair.value, &iv).unwrap();

        let ciphertext = cipher.encrypt_vec(&buffer);

        // let filename: String = randomizer::Randomizer::ALPHABETICAL(6).string().unwrap();
        // let path_name = format!("/tmp/{}", filename); // format path with random name
        // let file_path = path_name.as_str();

        // write ciphertext to file
        // let limit = 20 * 1024 *1024; // 20mb max
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        temp_file.write_all(&ciphertext);

        let file_path = temp_file.path().to_str().unwrap().to_string();

        let walrus_output = walrus_core::WalrusCore {}.store(&file_path);

        match walrus_output {
            Ok(json_str) => match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(json) => Some(json),
                Err(e) => {
                    eprintln!("Failed to parse JSON response: {}", e);
                    None
                }
            },
            Err(e) => {
                eprintln!("walrus command failed {}", e);
                None
            }
        }
    }
}
