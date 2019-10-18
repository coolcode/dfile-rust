use super::rocket;
use rocket::local::Client;

extern crate crypto;
use  super::crypto;
use crypto::digest::Digest;
use crypto::sha2::Sha256;


//fn hello_world() {
//    let rocket = rocket::ignite().mount("/", routes![super::hello]);
//    let client = Client::new(rocket).unwrap();
//    let mut response = client.get("/").dispatch();
//    assert_eq!(response.body_string(), Some("Hello, world!".into()));
//}

#[test]
fn sha256() {
    // create a Sha256 object
    let mut hasher = Sha256::new();

    // write input message
    hasher.input_str("hello world");

    // read hash digest
    let hex = hasher.result_str();

    assert_eq!(hex, concat!("b94d27b9934d3e08a52e52d7da7dabfa",  "c484efe37a5380ee9088f7ace2efcde9"));
}