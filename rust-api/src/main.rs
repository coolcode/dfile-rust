#![feature(proc_macro_hygiene, decl_macro)]
#![feature(plugin, custom_attribute)]

#[macro_use]
extern crate rocket;
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

struct FileData {
    name: String,
    content_type: String,
    buffer: Vec<u8>,
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

    println!("256 hash: {:?}", hex);
    let head = "12".to_owned();
    println!("head: {:?}", head);
    let ulen = hex.len();
    println!("ulen: {:?}", ulen);
    let size = "20".to_owned();

    let mut txt = String::from("");
    txt.push_str(&head);
    txt.push_str(&size);
    txt.push_str(&hex);
    println!("text: {:?}", txt);
    let hex_bytes = hex::decode(txt).expect("Error hex::decode");

    let encoded = bs58::encode(hex_bytes).into_string();
    println!("encoded: {:?}", encoded);
    Box::leak(encoded.into_boxed_str())
}

#[get("/")]
fn hello() -> &'static str {
    "Hello World!"
}

#[post("/", data = "<data>")]
// signature requires the request to have a `Content-Type` //, conf: State<'_, Config>
fn upload(content_type: &ContentType, data: Data) -> &'static str {
    // this and the next check can be implemented as a request guard but it seems like just
    // more boilerplate than necessary
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

    //let mut files = Vec::new();
    let mut multipart = Multipart::with_body(data.open(), boundary);
    //let mut file_data: FileData;
    let mut r = "";
    while let Some(entry) = multipart.read_entry().expect("Error in read entry") {
        let field_name = entry.headers.name.to_string();
        println!("field_name: {}", &field_name);
        let content_type = entry.headers.content_type.unwrap().to_string();
        //  .map(|mime| Mime::from_str(&mime.to_string()).unwrap());
        println!("content_type: {}", &content_type);
        let mut data = entry.data;
        let mut buffer: Vec<u8> = Vec::new();
        //data.stream_to(&mut buffer);
        //let mut buffer =  data.fill_buf().expect("Error in fil_buf").to_vec();
        //let mut buffer = io::BufRead::fill_buf(&data).expect("Error in fil_buf").to_vec();
        let mut sum_c = 0u64;
        let mut temp = [0u8; 4096];

        while let c = data.read(&mut temp).expect("Error in read data") {
            if c == 0 {
                break;
            }

            sum_c += c as u64;

            buffer.extend_from_slice(&temp[..c]);
        }

        println!("buffer: {}", buffer.len());
//        let mut file_data = FileData { name: field_name, content_type: content_type, buffer: buffer };
//        files.push(file_data);
//
//        let mut reader = file.buffer;
        r = upload_file_to_s3(&buffer);
        break;
    }

    return r;

//    match process_upload(boundary, data) {
//        Ok(&file) => {
//            //Ok(Stream::from(Cursor::new(resp)))
////            for file in files {
////                println!("file: {}", file.name);
////            }
////            let r;
////            if files.len() == 0 {
////                r = "No file!".to_owned();
////            } else {
////                let file = files.first().unwrap().buffer;
////
////
////                let mut reader = file.as_mut_slice();
////                //io::copy(&mut reader, &mut writer)?;
////
////                r = upload_file_to_s3(&reader).to_owned();
////            }
//
//            let mut reader = file.buffer;
//            let r = upload_file_to_s3(&reader).to_owned();
//            Ok(r)
//        }
//        Err(err) => Err(Custom(Status::InternalServerError, err.to_string()))
//    }
}

fn get_env_value(name: &str) -> String {
    env::var(name).expect(format!("No {:?} environment variable found", name).as_str())
}

fn upload_file_to_s3(contents: &[u8]) -> &'static str {
    let S3_REGION = get_env_value("S3_REGION");
    let S3_ENDPOINT = get_env_value("S3_ENDPOINT");
    let S3_KEY = get_env_value("S3_KEY");
    let S3_SECRET = get_env_value("S3_SECRET");

    println!("S3: {} {}", S3_REGION, S3_ENDPOINT);

    let region = Region::Custom {
        name: S3_REGION,
        endpoint: S3_ENDPOINT,
    };

    let start = Instant::now();
    println!("Starting up at {:?}", start);
    let filename = format!("{}.png", ipfs_hash(&contents));
    println!("[Hash] Took {:?}", Instant::now().duration_since(start));

    let s3client = S3Client::new_with(
        HttpClient::new().expect("failed to create request dispatcher"),
        StaticProvider::new_minimal(S3_KEY, S3_SECRET),
        region,
    );

    upload_file(&s3client, "test", &filename, &contents);
    println!("[Upload] Took {:?}", Instant::now().duration_since(start));

    Box::leak(filename.into_boxed_str())
}

//fn process_upload(boundary: &str, data: Data) -> io::Result<FileData> {
//    let mut files = Vec::new();
//    let mut multipart = Multipart::with_body(data.open(), boundary);
//    let mut file_data: FileData;
//    while let Some(entry) = multipart.read_entry()? {
//        let field_name = entry.headers.name.to_string();
//        println!("field_name: {}", &field_name);
//        let content_type = entry.headers.content_type.unwrap().to_string();
//        //  .map(|mime| Mime::from_str(&mime.to_string()).unwrap());
//        println!("content_type: {}", &content_type);
//        let mut data = entry.data;
//        let mut buffer: Vec<u8> = Vec::new();
//        //data.stream_to(&mut buffer);
//        //let mut buffer =  data.fill_buf().expect("Error in fil_buf").to_vec();
//        //let mut buffer = io::BufRead::fill_buf(&data).expect("Error in fil_buf").to_vec();
//        let mut sum_c = 0u64;
//        let mut temp = [0u8; 4096];
//
//        while let c = data.read(&mut temp)? {
//            if c == 0 {
//                break;
//            }
//
//            sum_c += c as u64;
//
//            buffer.extend_from_slice(&temp[..c]);
//        }
//
//        println!("buffer: {}", buffer.len());
//        file_data = FileData { name: field_name, content_type: content_type, buffer: buffer };
//        files.push(file_data);
//
//        //break;
//
////        if let Ok(vi) = options.allowed_fields.binary_search_by(|f| f.field_name.cmp(&field_name))
////        {
////            println!("vi: {}", vi);
////
////            //let mut buffer = [0u8; 4096];
////            //let field = options.allowed_fields.remove(vi);
////            match field.typ {
////                MultipartFormDataType::File => {
////                    let mut data = entry.data;
////                    let mut buffer: Vec<u8> = Vec::new();
////                    data.stream_to(&mut buffer);
////                    let file_data = FileData { name: field_name, content_type: content_type, buffer: buffer };
////                    files.push(file_data);
////                }
////            }
////        };
//    }
//
//    Ok(file_data)
//    //let mut out = Vec::new();
//    // let mut buffer: Vec<u8> = Vec::new();
////    // read the whole file
//////    let mut contents: Vec<u8> = Vec::new();
//////    //f.seek(std::io::SeekFrom::Start(0));
//////    match f.read_to_end(&mut contents) {
//////
//////    }
////    let f = data.open();
////    f.read_to_end(&mut buffer)?;
//    //  data.stream_to(&mut buffer);
//    //Ok(files)
//    // saves all fields, any field longer than 10kB goes to a temporary directory
//    // Entries could implement FromData though that would give zero control over
//    // how the files are saved; Multipart would be a good impl candidate though
////    match Multipart::with_body(data.open(), boundary).save().temp() {
////        Full(entries) => process_entries(entries, &mut out)?,
////        Partial(partial, reason) => {
////            writeln!(out, "Request partially processed: {:?}", reason)?;
////            if let Some(field) = partial.partial {
////                writeln!(out, "Stopped on field: {:?}", field.source.headers)?;
////            }
////
////            process_entries(partial.entries, &mut out)?
////        }
////        Error(e) => return Err(e),
////    }
////
////    Ok(out)
//}

// having a streaming output would be nice; there's one for returning a `Read` impl
// but not one that you can `write()` to
fn process_entries(entries: Entries, mut out: &mut Vec<u8>) -> io::Result<()> {
    {
        let stdout = io::stdout();
        let tee = StdoutTee::new(&mut out, &stdout);
        entries.write_debug(tee)?;
    }

    writeln!(out, "Entries processed")
}

fn upload_file(
    client: &S3Client,
    bucket: &str,
    dest_filename: &str,
    contents: &[u8],
) {
    let mut metadata = HashMap::new();
    metadata.insert("name".to_owned(), dest_filename.to_owned());

    let req = PutObjectRequest {
        bucket: bucket.to_owned(),
        key: dest_filename.to_owned(),
        body: Some(contents.to_owned().into()),
        content_type: Some("image/png".to_owned()),
        acl: Some("public-read".to_owned()),
        metadata: Some(metadata),
        ..Default::default()
    };
    let result = client.put_object(req).sync().expect("Couldn't PUT object");
    println!("{:#?}", result);
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
    dotenv().ok();
    rocket::ignite() //custom(config::config())
//        .attach(AdHoc::on_attach("conf", |rocket| {
//            println!("Adding s3 settings from config...");
//            let conf = rocket.config().clone();
//            Ok(rocket.manage( conf))
//        }))
        .mount("/", routes![upload, hello, hash])
        .register(catchers![not_found])
        .launch();
}
