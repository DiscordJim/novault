
use std::path::Path;

use anyhow::{Result, anyhow};
use chrono::Utc;
use reqwest::StatusCode;

pub fn get_snapshot_sig() -> String {
    Utc::now()
            .format("%Y%m%dT%H%M%SZ")
            .to_string()
            .parse()
            .unwrap()
}

pub fn t3_fetch(
    s3_access_key: &str,
    s3_secret_key: &str,
    bucket_name: &str,
    file_path: &Path
) -> Result<Vec<u8>> {


    let path_str = file_path.to_string_lossy();

    let datetime = chrono::Utc::now();
    let url = format!("https://{bucket_name}.t3.storage.dev/{path_str}");
    let mut headers = reqwest::header::HeaderMap::new();

    headers.insert(
        "X-Amz-Date",
        datetime
            .format("%Y%m%dT%H%M%SZ")
            .to_string()
            .parse()
            .unwrap(),
    );
    headers.insert("host", format!("{bucket_name}.t3.storage.dev").parse().unwrap());

    let s = aws_sign_v4::AwsSign::new(
        "GET",
        &url,
        &datetime,
        &headers,
        "us-east-1",
        &s3_access_key,
        &s3_secret_key,
        "execute-api",
        ""
    );
    let signature = s.sign();
    headers.insert(reqwest::header::AUTHORIZATION, signature.parse().unwrap());


    let body = reqwest::blocking::Client::new()
        .get(&url)
        .headers(headers)
        .send()?;

    Ok(body.bytes().unwrap().to_vec())
}


pub fn t3_delete(
    s3_access_key: &str,
    s3_secret_key: &str,
    bucket_name: &str,
    file_path: &Path
) -> Result<()> {


    let path_str = file_path.to_string_lossy();

    let datetime = chrono::Utc::now();
    let url = format!("https://{bucket_name}.t3.storage.dev/{}", path_str.replace("\\", "/"));
    let mut headers = reqwest::header::HeaderMap::new();

    // println!("Deleting: {url}");
    headers.insert(
        "X-Amz-Date",
        datetime
            .format("%Y%m%dT%H%M%SZ")
            .to_string()
            .parse()
            .unwrap(),
    );
    headers.insert("host", format!("{bucket_name}.t3.storage.dev").parse().unwrap());

    let s = aws_sign_v4::AwsSign::new(
        "DELETE",
        &url,
        &datetime,
        &headers,
        "us-east-1",
        &s3_access_key,
        &s3_secret_key,
        "execute-api",
        ""
    );
    let signature = s.sign();
    headers.insert(reqwest::header::AUTHORIZATION, signature.parse().unwrap());


    let body = reqwest::blocking::Client::new()
        .delete(&url)
        .headers(headers)
        .send()?;


    if body.status() != StatusCode::NO_CONTENT {
        return Err(anyhow!("Failed to get a 204 NO CONTENT."));
    }

    Ok(())
}

pub fn t3_put(
    s3_access_key: &str,
    s3_secret_key: &str,
    bucket_name: &str,
    file_path: &Path,
    data: Vec<u8>
) -> Result<()> {


    let path_str = file_path.to_string_lossy();

    let datetime = chrono::Utc::now();
    let url = format!("https://{bucket_name}.t3.storage.dev/{}", path_str.replace("\\", "/"));
    let mut headers = reqwest::header::HeaderMap::new();


    headers.insert(
        "X-Amz-Date",
        datetime
            .format("%Y%m%dT%H%M%SZ")
            .to_string()
            .parse()
            .unwrap(),
    );
    headers.insert("host", format!("{bucket_name}.t3.storage.dev").parse().unwrap());

    // headers.insert(reqwest::header::CONTENT_LENGTH, data.len().to_string().parse()?);
    headers.insert(reqwest::header::CONTENT_TYPE, "application/octet-stream".parse()?);
    

    let s = aws_sign_v4::AwsSign::new(
        "PUT",
        &url,
        &datetime,
        &headers,
        "us-east-1",
        &s3_access_key,
        &s3_secret_key,
        "s3",
        ""
    );
    let signature = s.sign();
    headers.insert(reqwest::header::AUTHORIZATION, signature.parse().unwrap());



    // println!("CHANGE: {:?}", body);

    let body = reqwest::blocking::Client::new()
        .put(&url)
        .headers(headers)
        .body(data)
        // .build()?;

    // println!("BODY: {:?}", body);
        
        .send()?;


    if body.status() != StatusCode::OK {
        return Err(anyhow!("Failed to get a 200 OK response from the put got {:?}", body.status()));
    }

    // println!("BODY: {:?}", body.text()?);


    Ok(())
}

