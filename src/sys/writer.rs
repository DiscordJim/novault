use std::{fs::File, io::Cursor, path::{Path, PathBuf}};

use aes_gcm::{KeyInit, aead::AeadMutInPlace};
use anyhow::{Result, anyhow};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use zip::{ZipWriter, write::FileOptions};
use std::io::Write;


pub struct VaultWriter {
    // encryptor: EncryptorBE32<XChaCha20Poly1305>,
    file: Option<ZipWriter<Cursor<Vec<u8>>>>,
    options: FileOptions<'static, ()>,
    path: PathBuf,
    key: [u8; 32]
}

impl VaultWriter {
    pub fn new(target: impl AsRef<Path>, key: &[u8; 32])  -> Result<Self> {

        let path = target.as_ref();
        let file = Cursor::new(vec![]);



        let enc_options: FileOptions<'_, ()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);


        let file = ZipWriter::new(file);



        Ok(Self {
            options: enc_options,
            file: Some(file),
            key: *key,
            path: path.to_path_buf()
        })
    }
    pub fn write_path(&mut self, path: &Path, name: &Path) -> Result<()> {
        if let Some(file) = &mut self.file {
            if path.is_file() {
            file.start_file(name.to_string_lossy(), self.options)?;
            file.write_all(&std::fs::read(path)?)?;
        } else {
            file.add_directory(name.to_string_lossy(), self.options)?;
        }
        } else {
            return Err(anyhow!("Failed to actually get the file innards."));
        }
        


        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {

        let file = self.file.take().unwrap();

        let mut inner = file.finish()?.into_inner();

        let key = Key::from_slice(&self.key);
        let mut cipher = XChaCha20Poly1305::new(&key);


        let mut nbytes = [0u8; 24];
        rand::fill(&mut nbytes);


        // Reserve some space.
        inner.reserve(16);

        let nonce = XNonce::from_slice(&nbytes);

        cipher.encrypt_in_place(nonce, &[], &mut inner)
            .map_err(|_| anyhow!("Failed to encrypt the vault contents in place."))?;

        let mut file = File::create(&self.path)?;
        file.write_all(b"NOVO")?;
        file.write_all(&[0u8, 0, 0, 0])?;
        file.write_all(&nonce)?;
        file.write_all(&inner)?;

        
        // std::fs::write(&self.path, &inner)?;


        Ok(())
    }
}



pub fn decrypt(
    header: &mut Vec<u8>,
    vault: &mut Vec<u8>,
    key: &[u8; 32]  
) -> Result<()> {

    // let split = bytes.spl


    if header[..4] != *b"NOVO" {
        return Err(anyhow!("Could not find the magic header at the top of the vault binary."));
    }


    // let nonce_slice = 

    // let nonce_slice = bytes[8..8 + 24].to_vec();

    let nonce = XNonce::from_slice(&header[8..]);


    let key= Key::from_slice(key);

    let mut cipher = XChaCha20Poly1305::new(key);

    cipher.decrypt_in_place(nonce, &[], vault)
        .unwrap();




    Ok(())
}