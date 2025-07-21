use serde::{Deserialize, Serialize};

use crate::model_core::key::Algorithm;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenerateKeypairRequest {
    pub owner: String,
    pub alias: String,        // MongoDB ObjectId as string
    pub algorithm: Algorithm, // e.g., "AES", "RSA", etc.
}
