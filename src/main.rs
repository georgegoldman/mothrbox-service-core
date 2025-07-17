#[macro_use]
extern crate rocket;

use dotenv::dotenv;
use rocket::{
    data::ByteUnit,
    figment::{
        util::map,
        value::{Map, Value},
        Figment,
    },
};
use rocket_cors::{AllowedOrigins, CorsOptions};
use std::env;
use std::process::Command;

mod api_core;
mod db;
mod dto;
mod encryption_core;
mod middleware;
mod model_core;
mod paste_id;
mod piston;
mod sui_core;
mod walrus_core;

#[catch(413)]
fn too_large(_req: &rocket::Request<'_>) -> &'static str {
    "File too large. Max size is 30GB."
}

#[launch]
async fn rocket() -> _ {
    // Print system info (optional)
    let uname = Command::new("uname").arg("-a").output().unwrap();
    println!("🖥️ OS Info: {}", String::from_utf8_lossy(&uname.stdout));

    let arch = Command::new("arch").output().unwrap();
    println!("🧱 Architecture: {}", String::from_utf8_lossy(&arch.stdout));

    // Load environment variables
    dotenv().ok();

    // Load DB collections
    let token_collection = db::connect::<model_core::ApiToken>().await;
    let key_pair = db::connect::<model_core::KeyPair>().await;

    // CORS config
    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .to_cors()
        .unwrap();

    // Load port from .env or default to 7000
    let port = env::var("PORT")
        .unwrap_or_else(|_| "7000".to_string())
        .parse::<u16>()
        .expect("Invalid PORT number");

    // Merge Rocket config with 30GB form & file limit
    let figment: Figment = rocket::Config::figment()
        .merge((
            "limits",
            map! {
                "form" => Value::from(30 * 1024 * 1024 * 1024u64),  // 30 GB
                "data-form" => Value::from(ByteUnit::Gibibyte(30).as_u64()), // important for now
                "file" => Value::from(30 * 1024 * 1024 * 1024u64),  // 30 GB
                "file/$ext" => Value::from(30 * 1024 * 1024 * 1024u64),  // 30 GB
                "string" => Value::from(30 * 1024 * 1024 * 1024u64),  // 30 GB
                "bytes" => Value::from(30 * 1024 * 1024 * 1024u64),  // 30 GB
                "json" => Value::from(30 * 1024 * 1024 * 1024u64),  // 30 GB
                "msgpack" => Value::from(30 * 1024 * 1024 * 1024u64),  // 30 GB
            },
        ))
        .merge(("address", "0.0.0.0"))
        .merge(("port", port));

    // Sanity check: print limits
    let config = rocket::Config::from(&figment);
    println!(
        "✅ Final form limit: {} bytes",
        config.limits.get("form").unwrap()
    );
    println!(
        "✅ Final file limit: {} bytes",
        config.limits.get("file").unwrap()
    );

    // Launch Rocket
    rocket::custom(figment)
        .register("/", catchers![too_large])
        .attach(cors)
        .manage(token_collection)
        .manage(key_pair)
        .mount(
            "/engine/core",
            routes![
                api_core::issue_token,
                api_core::walrus_test,
                api_core::create_key,
                api_core::encrypt,
                api_core::decrypt,
                api_core::sui_service,
            ],
        )
}
