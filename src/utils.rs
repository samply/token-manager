use aes::Aes256; 
use ctr::Ctr128BE; 
use cipher::{KeyIvInit, StreamCipher};
use base64::{engine::general_purpose::STANDARD, Engine};
use crate::config::CONFIG;


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
    let decrypted_token =  encrypt_data(&toke_decode, nonce);
    let token_str = String::from_utf8(decrypted_token).unwrap();
    token_str 
}

pub fn generate_r_script(script_lines: Vec<String>) -> String {
    let mut builder_script = String::from(
        r#"library(DSI)
library(DSOpal)
library(dsBaseClient)
set_config(use_proxy(url="http://beam-connect", port=8062))
set_config( config( ssl_verifyhost = 0L, ssl_verifypeer = 0L ) )

builder <- DSI::newDSLoginBuilder(.silent = FALSE)
"#,
    );

    // Append each line to the script.
    for line in script_lines {
        builder_script.push_str(&line);
        builder_script.push('\n');
    }

    // Finish the script with the login and assignment commands.
    builder_script.push_str(
        "logindata <- builder$build()
connections <- DSI::datashield.login(logins = logindata, assign = TRUE, symbol = 'D')\n",
    );

    builder_script
}



