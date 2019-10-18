extern crate rocket;

use rocket::config::{Config, Environment, Value};
use std::env;
use std::collections::HashMap;

pub fn config() -> Config {
    let environment = Environment::active().expect("No environment found");

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse::<u16>()
        .expect("PORT environment variable should parse to an integer");


    //let env_keys = vec!["LOGDNA_KEY", "S3_REGION", "S3_ENDPOINT", "S3_KEY", "S3_SECRET", "INIT_FILE_COUNT"];
    let secret_key = env::var("SECRET_KEY").expect("No SECRET_KEY environment variable found");

    let mut conf = Config::build(environment)
        .environment(environment)
        .port(port)
        .secret_key(secret_key)
        ;

    for (key, value) in env::vars() {
        conf = conf.extra(&key, value);
    }
//    for key in &env_keys {
//        let value = env::var(*key).expect(format!("No '{}' environment variable found", *key).as_str());
//        conf = conf.extra(*key, value);
//    }

    conf.finalize().unwrap()
}