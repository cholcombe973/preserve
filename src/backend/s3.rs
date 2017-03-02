
use std::env::home_dir;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::path::PathBuf;

use backend::Backend;
use error::*;
use keystore::{EncryptedArchiveName, EncryptedArchive, EncryptedBlock, BlockId};
use rustc_serialize::json;
use s3::{Bucket, put_s3, get_s3, list_s3};

/// Backup to a Ceph cluster
pub struct S3 {
    bucket: Bucket,
}

#[derive(RustcDecodable)]
struct S3Config {
    /// The bucket to store backups in
    bucket_name: String,
    /// The AWS region
    region: String,
    /// The AWS access key
    aws_access: String,
    /// The AWS secret key
    aws_secret: String,
}

impl S3 {
    pub fn new(config_dir: Option<PathBuf>) -> Result<S3> {
        let s3_config: S3Config = match config_dir {
            Some(config) => {
                info!("Reading s3 config file: {}/{}", config.display(), "s3.json");
                let mut f = try!(File::open(config.join("s3.json")));
                let mut s = String::new();
                try!(f.read_to_string(&mut s));
                try!(json::decode(&s))
            }
            None => {
                info!("Reading s3 config file: {}/{}",
                      home_dir().unwrap().to_string_lossy(),
                      ".config/s3.json");
                let mut f = try!(File::open(format!("{}/{}",
                                                    home_dir().unwrap().to_string_lossy(),
                                                    ".config/s3.json")));
                let mut s = String::new();
                try!(f.read_to_string(&mut s));
                try!(json::decode(&s))
            }
        };
        info!("Connecting to S3");
        let credentials = Credentials::new(&s3_config.aws_access, &s3_config.aws_secret, None);
        let region = try!(s3_config.region.parse());
        let bucket = Bucket::new(BUCKET, region, credentials);

        Ok(S3 { bucket: bucket })
    }
}

impl Backend for S3 {
    fn block_exists(&mut self, id: &BlockId) -> Result<bool> {
        let (get, code) = try!(self.bucket.get(&id.to_string()));
        match code {
            // Doesn't exist
            404 => return Ok(false),
            200 => return Ok(true),
            // Some other error happened
            _ => return Ok(false),
        }
    }

    fn store_block(&mut self,
                   id: &BlockId,
                   &EncryptedBlock(ref data): &EncryptedBlock)
                   -> Result<()> {
        put_s3(&self.bucket, &id.to_string(), &data);
        Ok(())
    }

    fn fetch_block(&mut self, id: &BlockId) -> Result<EncryptedBlock> {
        // This should be no larger than 1MB so it's ok to read to memory
        let (ciphertext, code) = try!(self.bucket.get(&id.to_string()));
        match code {
            200 => {
                debug!("Read {} bytes from s3 for fetch_block", ciphertext.len());
                Ok(EncryptedBlock(ciphertext))
            }
            // Some error happened
            _ => Err(""),
        }
    }

    fn fetch_archive(&mut self, name: &EncryptedArchiveName) -> Result<EncryptedArchive> {
        let ciphertext = try!(get_s3(&self.bucket, Some(&name.to_string())));
        debug!("Read {} bytes from s3 for fetch_archive", ciphertext.len());
        Ok(EncryptedArchive(ciphertext))
    }

    fn store_archive(&mut self,
                     name: &EncryptedArchiveName,
                     &EncryptedArchive(ref payload): &EncryptedArchive)
                     -> Result<()> {
        let (_, code) = try!(bucket.put(&name.to_string(), payload, "text/plain"));
        match code {
            200 => Ok(()),
            _ => {
                // Decode error
                Err("")
            }
        }
    }

    fn list_archives(&mut self) -> Result<Vec<EncryptedArchiveName>> {
        let mut archives = Vec::new();

        // List out contents of directory
        let (list, code) = bucket.list("metadata", None).unwrap();
        match code {
            200 => {
                for object in list.contents {
                    let encrypted_archive_name =
                        match EncryptedArchiveName::from_str(&object.key) {
                            Ok(name) => name,
                            Err(_) => return Err(Error::InvalidArchiveName),
                        };
                    archives.push(encrypted_archive_name);
                }
            }
            _ => {
                // Decode error
                return Err("");
            }
        };

        Ok(archives)
    }
}
