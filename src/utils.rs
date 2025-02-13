use std::{fs, io};
use std::collections::{HashMap, HashSet};
use crate::config::CONFIG;
use aes::Aes256;
use base64::{engine::general_purpose::STANDARD, Engine};
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;

fn adjust_key(key: &str) -> [u8; 32] {
    let bytes = key.as_bytes();
    let mut array = [0u8; 32];
    let bytes_to_copy = bytes.len().min(32);
    array[..bytes_to_copy].copy_from_slice(&bytes[..bytes_to_copy]);

    array
}

pub fn encrypt_data(data: &[u8], nonce: &[u8]) -> Vec<u8> {
    let key: [u8; 32] = adjust_key(&CONFIG.token_encrypt_key);

    let mut cipher = Ctr128BE::<Aes256>::new_from_slices(&key, nonce).unwrap();
    let mut encrypted = data.to_vec();
    cipher.apply_keystream(&mut encrypted);

    encrypted
}

pub fn decrypt_data(data: String, nonce: &[u8]) -> String {
    let toke_decode = STANDARD.decode(data).unwrap();
    let decrypted_token = encrypt_data(&toke_decode, nonce);
    String::from_utf8(decrypted_token).unwrap()
}

pub fn generate_r_script(script_config: String) -> Result<String, io::Error> {
    // Read the auth script template from the file
    let template_path = &CONFIG.auth_script_template_path;
    let template_content = fs::read_to_string(template_path)?;

    // Replace the placeholder with the credentials CSV
    let processed_content = template_content.replace("${CSV_CREDENTIALS_CONFIG}", script_config.as_str());

    // Return the processed content
    Ok(processed_content)
}

pub fn fetch_tables_prefix(
    bridgehead_tables: &HashMap<String, HashSet<String>>, bridgehead: &str, default: &str) -> String {
    if let Some(set) = bridgehead_tables.get(bridgehead) {
        let mut prefix: Option<String> = None;

        for value in set {
            if let Some((current_prefix, _)) = value.split_once('.') {
                match &prefix {
                    Some(existing_prefix) if existing_prefix != current_prefix => {
                        return default.to_string();
                    }
                    None => {
                        prefix = Some(current_prefix.to_string());
                    }
                    _ => {}
                }
            }
        }
        prefix.unwrap_or_else(|| default.to_string())
    } else {
        default.to_string()
    }
}

