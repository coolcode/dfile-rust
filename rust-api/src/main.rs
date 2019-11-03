#![feature(proc_macro_hygiene, decl_macro)]
#![feature(plugin, custom_attribute)]


#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
extern crate multipart;
extern crate rusoto_core;
extern crate rusoto_s3;
extern crate crypto;
extern crate bs58;
extern crate hex;
extern crate dotenv;


#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::Read;
use std::str;
use std::time::{Duration, Instant};
use std::io::{self, Cursor, Write, Seek, SeekFrom};
use std::collections::HashMap;

use multipart::mock::StdoutTee;
use multipart::server::Multipart;
use multipart::server::save::Entries;
use multipart::server::save::SaveResult::*;

use rocket::http::{ContentType, Status};
use rocket::{Request, State, Data, Response};
use rocket::response::Stream;
use rocket::response::status::Custom;

use rusoto_core::credential::{AwsCredentials, DefaultCredentialsProvider, StaticProvider};
use rusoto_core::request::HttpClient;
use rusoto_core::Region;
use rusoto_s3::{S3, S3Client, PutObjectRequest};

use crypto::digest::Digest;
use crypto::sha2::Sha256;

use std::env;
use dotenv::dotenv;

mod config;

use rocket::config::Config;
use rocket::fairing::AdHoc;

extern crate url;

use url::form_urlencoded::{byte_serialize, parse};
use rocket_contrib::json::{Json, JsonValue};
use std::sync::atomic::{AtomicUsize, Ordering};


#[macro_use]
extern crate log;
extern crate log4rs;

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::append::console::ConsoleAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Logger, Root};


struct FileData {
    name: String,
    content_type: String,
    buffer: Vec<u8>,
}

struct UploadCount {
    count: AtomicUsize
}

#[get("/hash")]
fn hash() -> &'static str {
    ipfs_hash("hello world".as_bytes())
}


fn ipfs_hash(contents: &[u8]) -> &'static str {
    let mut hasher = Sha256::new();
    hasher.input(contents);

    // read hash digest
    let hex = hasher.result_str();

    info!("256 hash: {:?}", hex);
    let head = "12".to_owned();
    info!("head: {:?}", head);
    let ulen = hex.len();
    info!("ulen: {:?}", ulen);
    let size = "20".to_owned();

    let mut txt = String::from("");
    txt.push_str(&head);
    txt.push_str(&size);
    txt.push_str(&hex);
    info!("text: {:?}", txt);
    let hex_bytes = hex::decode(txt).expect("Error hex::decode");

    let encoded = bs58::encode(hex_bytes).into_string();
    info!("encoded: {:?}", encoded);
    Box::leak(encoded.into_boxed_str())
}

#[get("/")]
fn hello() -> &'static str {
    "Hello World!"
}

#[get("/stat")]
fn stat(upload_count: State<UploadCount>) -> JsonValue {
    let init_file_count = get_env_value("INIT_FILE_COUNT").parse::<u32>().unwrap();
    let current_count_usize = upload_count.count.load(Ordering::Relaxed);
    let mut s = format!("{}", current_count_usize);
    let current_count = u32::from_str_radix(&s,10).unwrap();
    info!("current_count: {}", current_count);
    let count = current_count + init_file_count;
    json!({ "file_count": count  })
}

#[post("/", data = "<data>")]
// signature requires the request to have a `Content-Type`
fn upload(content_type: &ContentType, data: Data, upload_count: State<UploadCount>) -> &'static str {
    if !content_type.is_form_data() {
//        return Err(Custom(
//            Status::BadRequest,
//            "Content-Type not multipart/form-data".into(),
//        ));
        return "Content-Type not multipart/form-data";
    }

    let (_, boundary) = content_type.params().find(|&(k, _)| k == "boundary").ok_or_else(
//        || Custom(
//            Status::BadRequest,
//            "`Content-Type: multipart/form-data` boundary param not provided".into(),
//        )
|| "`Content-Type: multipart/form-data` boundary param not provided"
    ).expect("Error in boundary");

    let mut multipart = Multipart::with_body(data.open(), boundary);
    let mut r = "";
    while let Some(entry) = multipart.read_entry().expect("Error in read entry") {
        let field_name = entry.headers.name.to_string();
        info!("field_name: {}", &field_name);
        let filename = entry.headers.filename.unwrap();
        let encoded_filename: String = byte_serialize(filename.as_bytes()).collect();
        info!("filename: {}", &encoded_filename);
        let content_type = entry.headers.content_type.unwrap().to_string();
        info!("content_type: {}", &content_type);
        let mut buffer: Vec<u8> = Vec::new();
        let mut temp = [0u8; 4096];
        let mut entry_data = entry.data;
        while let c = entry_data.read(&mut temp).expect("Error in read data") {
            if c == 0 {
                break;
            }
            buffer.extend_from_slice(&temp[..c]);
        }

        info!("buffer: {}", buffer.len());
        r = upload_file_to_s3(&buffer, &encoded_filename, &content_type);
        //TODO: multiple files
        break;
    }

    info!("r: {}", r);

    upload_count.count.fetch_add(1, Ordering::SeqCst);

    return r;
}

fn get_env_value(name: &str) -> String {
    env::var(name).expect(format!("No {:?} environment variable found", name).as_str())
}

fn upload_file_to_s3(contents: &[u8], meta_filename: &str, content_type: &str) -> &'static str {
    let S3_REGION = get_env_value("S3_REGION");
    let S3_ENDPOINT = get_env_value("S3_ENDPOINT");
    let S3_KEY = get_env_value("S3_KEY");
    let S3_SECRET = get_env_value("S3_SECRET");

    info!("S3: {} {}", S3_REGION, S3_ENDPOINT);

    let region = Region::Custom {
        name: S3_REGION,
        endpoint: S3_ENDPOINT,
    };

    let start = Instant::now();
    info!("Starting up at {:?}", start);
    let filename = format!("{}.png", ipfs_hash(&contents));
    info!("[Hash] Took {:?}", Instant::now().duration_since(start));

    let s3client = S3Client::new_with(
        HttpClient::new().expect("failed to create request dispatcher"),
        StaticProvider::new_minimal(S3_KEY, S3_SECRET),
        region,
    );

    upload_file(&s3client, "test", &filename, &contents, &meta_filename, &content_type);
    info!("[Upload] Took {:?}", Instant::now().duration_since(start));

    Box::leak(filename.into_boxed_str())
}

fn upload_file(
    client: &S3Client,
    bucket: &str,
    dest_filename: &str,
    contents: &[u8],
    meta_filename: &str,
    content_type: &str,
) {
    let mut metadata = HashMap::new();
    metadata.insert("name".to_owned(), meta_filename.to_owned());

    let req = PutObjectRequest {
        bucket: bucket.to_owned(),
        key: dest_filename.to_owned(),
        body: Some(contents.to_owned().into()),
        content_type: Some(content_type.to_owned()),
        acl: Some("public-read".to_owned()),
        metadata: Some(metadata),
        ..Default::default()
    };
    let result = client.put_object(req).sync().expect("Couldn't PUT object");
    info!("{:#?}", result);
}

fn read_content(local_filename: &str) -> Vec<u8> {
    let mut f = File::open(local_filename).unwrap();
    let mut contents: Vec<u8> = Vec::new();
    f.read_to_end(&mut contents);
    contents
}

#[catch(404)]
fn not_found() -> &'static str {
    "404: Resource was not found."
}

struct S3Config {
    region: &'static str,
    endpoint: &'static str,
    key: &'static str,
    secret: &'static str,
}

fn main() {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
//    let mut format = "{l} - {m}\n";
//
//    let stdout = ConsoleAppender::builder()
//        .encoder(Box::new(PatternEncoder::new(&format)))
//        .build();
//
//    let logfile = FileAppender::builder()
//        .encoder(Box::new(PatternEncoder::new(&format)))
//        .build("logs/app.log")
//        .unwrap();
//
//    let config = log4rs::config::Config::builder()
//        .appender(Appender::builder().build("stdout", Box::new(stdout)))
//        .appender(Appender::builder().build("logfile", Box::new(logfile)))
//        //.logger(Logger::builder().appender("logfile").build("app::info",LevelFilter::Info))
//        .build(Root::builder()
//            .appender("stdout")
//            .appender("logfile")
//            .build(LevelFilter::Info)
//        )
//        .unwrap();
//
//    log4rs::init_config(config).unwrap();

    info!("Hello, world!");

    dotenv().ok();
    rocket::ignite() //custom(config::config())
//        .attach(AdHoc::on_attach("conf", |rocket| {
//            info!("Adding s3 settings from config...");
//            let conf = rocket.config().clone();
//            Ok(rocket.manage( conf))
//        }))
        .manage(UploadCount { count: AtomicUsize::new(0) })
        .mount("/", routes![upload, hello, hash, stat])
        .register(catchers![not_found])
        .launch();
}
