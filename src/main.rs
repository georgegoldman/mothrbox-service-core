#[macro_use]
extern crate rocket;
use std::path::Path;
// use encryption_core::new_encryption;
use dotenv::dotenv;
use rocket::data::ByteUnit;
use std::env;
// use route::post_video;
use std::process::Command;



mod encryption_core;
mod paste_id;
mod model_core;
mod api_core;
mod db;
mod dto;
mod middleware;
mod sui_core;
mod walrus_core;
mod piston;

use rocket_cors::{AllowedOrigins, CorsOptions};

#[catch(413)]
fn too_large(_req: &rocket::Request<'_>) -> &'static str {
     "File too large Max size is 3GB"
}


#[launch]
async fn rocket() -> _ {
         let uname = Command::new("uname").arg("-a").output().unwrap();
    println!("OS Info: {}", String::from_utf8_lossy(&uname.stdout));

    let arch = Command::new("arch").output().unwrap();
    println!("Architecture: {}", String::from_utf8_lossy(&arch.stdout));
     dotenv().ok();

     // connect the different database collections
     let token_collection = db::connect::<model_core::ApiToken>().await;
     let key_pair = db::connect::<model_core::KeyPair>().await;
     // let keypair_collection = db::connect::<new_encryption::KeyPairDocument>().await;

     // Create your EccKeyManager from the keypair_collection
//     let key_manager = new_encryption::EccKeyManager::from_collection(keypair_collection);
     

     let port  = env::var("PORT")
          .unwrap_or_else(|_| "7000".to_string())
          .parse::<u16>()
          .expect("Invalid PORT number");
     let cors = CorsOptions::default()
     .allowed_origins(AllowedOrigins::all())
     .to_cors()
     .unwrap();

     let config = rocket::Config::figment();
     let form_limit: u64 = config.extract_inner("limits.form").unwrap_or(0);
     let file_limit: u64 = config.extract_inner("limits.file").unwrap_or(0);
     println!("🚀 Form limit: {} bytes", form_limit);
     println!("📁 File limit: {} bytes", file_limit);
     let limits = rocket::data::Limits::new()
          .limit("form", ByteUnit::Gibibyte(30)) // 30 GiB
          .limit("file", ByteUnit::Gibibyte(30)); // ,, ,,

     let figment = rocket::Config::figment().merge(("limits", limits))
     .merge(("address", "0.0.0.0"))
     .merge(("port", 7000));

     rocket::custom(figment)
     .register("/", catchers![too_large])
     .attach(cors)
     .manage(token_collection)
     .manage(key_pair)
     // .manage(key_manager)
     .mount("/engine/core", routes![
          api_core::issue_token,
          api_core::walrus_test,
          // api_core::spawn_user,
          api_core::create_key,
          api_core::encrypt,
          api_core::decrypt,
          // api_core::key_exists,
          // api_core::sign_message,
          // api_core::verify_signature,  
          ])
     
}