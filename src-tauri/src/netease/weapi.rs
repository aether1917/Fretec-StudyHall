//! 网易云 weapi 参数加密。
//!
//! 全部为纯函数，不发起网络请求，便于单元测试：
//! - `params` = base64(AES-128-CBC(key2, AES-128-CBC(key1, json)))
//! - `encSecKey` = RSA_NO_PADDING(reverse(key2))，固定 256 位十六进制字符串
//!
//! 注意：`encSecKey` 使用的是网易云固定 1024 位公钥做裸模幂运算
//! （无 PKCS#1 v1.5 / OAEP 填充），因此这里用 `num-bigint` 直接计算。

use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use num_bigint::BigUint;
use rand::Rng;

use crate::error::{AppError, CmdResult};

#[cfg(test)]
use aes::cipher::BlockDecryptMut;

/// 网易云 weapi 固定 AES 密钥。
pub(crate) const AES_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
/// 网易云 weapi 固定 IV。
pub(crate) const AES_IV: &[u8; 16] = b"0102030405060708";

/// 网易云固定 RSA 公钥指数。
const RSA_PUBLIC_EXPONENT: &str = "010001";
/// 网易云固定 RSA 公钥模数（1024 位）。
const RSA_MODULUS: &str = concat!(
    "00e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a",
    "5aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685",
    "b7a46bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462d",
    "b0a22b8e7",
);

/// weapi 请求所需的两个表单字段。
#[derive(Debug, Clone)]
pub(crate) struct WeapiForm {
    /// 两轮 AES 加密后的 base64 字符串。
    pub(crate) params: String,
    /// RSA 裸加密结果，256 位十六进制。
    pub(crate) enc_sec_key: String,
}

impl WeapiForm {
    /// 构造 `application/x-www-form-urlencoded` 请求体。
    pub(crate) fn body(&self) -> String {
        format!(
            "params={}&encSecKey={}",
            urlencoding::encode(&self.params),
            self.enc_sec_key
        )
    }
}

/// 对 JSON 参数执行完整的 weapi 加密，随机生成 key2。
pub(crate) fn encrypt_payload(payload: &serde_json::Value) -> CmdResult<WeapiForm> {
    let plaintext =
        serde_json::to_string(payload).map_err(|_| AppError::msg("网易云请求参数生成失败"))?;
    let key2 = random_key2();
    encrypt_plain_with_key(&plaintext, &key2)
}

/// 用指定 key2 加密明文，供单测回环验证。
fn encrypt_plain_with_key(plaintext: &str, key2: &[u8; 16]) -> CmdResult<WeapiForm> {
    let first = aes_encrypt_base64(plaintext.as_bytes(), AES_KEY)?;
    let params = aes_encrypt_base64(first.as_bytes(), key2)?;
    let enc_sec_key = rsa_encrypt_no_padding(key2)?;
    Ok(WeapiForm {
        params,
        enc_sec_key,
    })
}

/// 生成 16 字节随机 key2（字母数字，保证逐字节可逆且 ASCII）。
fn random_key2() -> [u8; 16] {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    let mut key = [0u8; 16];
    for slot in key.iter_mut() {
        *slot = ALPHABET[rng.gen_range(0..ALPHABET.len())];
    }
    key
}

/// AES-128-CBC + PKCS7，返回 base64 字符串。
fn aes_encrypt_base64(data: &[u8], key: &[u8; 16]) -> CmdResult<String> {
    let encryptor = cbc::Encryptor::<aes::Aes128>::new_from_slices(key, AES_IV)
        .map_err(|_| AppError::msg("网易云请求加密失败"))?;
    // PKCS7 至少填充 1 字节，最多 16 字节，预留一个完整块即可。
    let mut buffer = vec![0u8; data.len() + 16];
    buffer[..data.len()].copy_from_slice(data);
    let encrypted = encryptor
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, data.len())
        .map_err(|_| AppError::msg("网易云请求加密失败"))?;
    Ok(STANDARD.encode(encrypted))
}

/// AES-128-CBC 解密，仅用于单测验证加密实现。
#[cfg(test)]
fn aes_decrypt_base64(encoded: &str, key: &[u8; 16]) -> CmdResult<String> {
    let mut buffer = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::msg("网易云测试数据解码失败"))?;
    let decryptor = cbc::Decryptor::<aes::Aes128>::new_from_slices(key, AES_IV)
        .map_err(|_| AppError::msg("网易云测试解密初始化失败"))?;
    let plain = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|_| AppError::msg("网易云测试解密失败"))?;
    String::from_utf8(plain.to_vec()).map_err(|_| AppError::msg("网易云测试明文不是合法的 UTF-8"))
}

/// 网易云 encSecKey：把 key2 反转后做 RSA 裸模幂，输出 256 位十六进制。
fn rsa_encrypt_no_padding(secret_key: &[u8; 16]) -> CmdResult<String> {
    let modulus = BigUint::parse_bytes(RSA_MODULUS.as_bytes(), 16)
        .ok_or_else(|| AppError::msg("网易云加密公钥无效"))?;
    if modulus == BigUint::from(0u32) {
        return Err(AppError::msg("网易云加密公钥无效"));
    }
    let exponent = BigUint::parse_bytes(RSA_PUBLIC_EXPONENT.as_bytes(), 16)
        .ok_or_else(|| AppError::msg("网易云加密公钥无效"))?;

    let mut reversed = *secret_key;
    reversed.reverse();
    let message = BigUint::from_bytes_be(&reversed);
    let mut hex = message.modpow(&exponent, &modulus).to_str_radix(16);

    if hex.len() < 256 {
        let mut padded = String::with_capacity(256);
        for _ in 0..(256 - hex.len()) {
            padded.push('0');
        }
        padded.push_str(&hex);
        hex = padded;
    } else if hex.len() > 256 {
        let start = hex.len() - 256;
        hex = hex.split_off(start);
    }
    Ok(hex)
}

/// 密码登录使用：先对密码做一次 MD5 十六进制摘要。
pub(crate) fn md5_hex(value: &str) -> String {
    format!("{:x}", md5::compute(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_double_layer_round_trip_with_same_key2() {
        let key2 = *b"Ab3dEf6hIj9kLm2n";
        let plaintext = r#"{"s":"周杰伦","type":1,"limit":20,"offset":0,"total":true}"#;
        let form = encrypt_plain_with_key(plaintext, &key2).expect("weapi 加密应成功");
        assert_eq!(form.enc_sec_key.len(), 256);

        // 先用 key2 解外层，再用固定 key1 解内层，必须还原原始 JSON。
        let inner = aes_decrypt_base64(&form.params, &key2).expect("外层解密应成功");
        let decoded = aes_decrypt_base64(&inner, AES_KEY).expect("内层解密应成功");
        assert_eq!(decoded, plaintext);
    }

    #[test]
    fn enc_sec_key_is_256_hex_chars() {
        let key2 = *b"0123456789abcdef";
        let first = rsa_encrypt_no_padding(&key2).expect("RSA 加密应成功");
        let second = rsa_encrypt_no_padding(&key2).expect("RSA 加密应成功");
        assert_eq!(first.len(), 256);
        assert!(first
            .chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch)));
        // 同一 key2 结果必须确定，前端/服务端才可复现。
        assert_eq!(first, second);
        // 与 Python BigInteger 独立实现比对，锁定“反转 + 裸模幂”算法。
        assert_eq!(
            first,
            "35701388baf89fed412e11269b9c76625d095ecaf17f03fa018abe19ea2d38b949debf242ee39a71ca1f6cda71b1b86a45aa909ee27f7e78e267d34e732f0de948206c3340a788d0003372183e2f753c1f78b66ac23d134ac1fc9b993156520ea826b8aa89a962d4491b4b8d7e08738e1da9b07aa39bf4a7ef0b1c210728cd52"
        );
    }

    #[test]
    fn weapi_known_vector_matches_independent_implementation() {
        // 期望值由 Node.js crypto(aes-128-cbc/PKCS7) 与 Python BigInteger
        // 两次独立计算得到，用于锁定 key1/iv/双层顺序/key2 反转等细节。
        let key2 = *b"0123456789abcdef";
        let plaintext = r#"{"s":"周杰伦","type":1,"limit":20,"offset":0,"total":true}"#;
        let form = encrypt_plain_with_key(plaintext, &key2).expect("weapi 加密应成功");
        assert_eq!(
            form.params,
            "DxVOVqI5w4fJyAFk0z/oxO9nOWgFu1HsnlZgdXIUuncBsJZjpDQeYR8Tma73Uk+mXB7nUfJBfUAoRi0RXAKiuRC18SBzbQw3geRKgCiIRFrVpZqxL6bGT995zS3SSTXY"
        );
        assert_eq!(
            form.enc_sec_key,
            "35701388baf89fed412e11269b9c76625d095ecaf17f03fa018abe19ea2d38b949debf242ee39a71ca1f6cda71b1b86a45aa909ee27f7e78e267d34e732f0de948206c3340a788d0003372183e2f753c1f78b66ac23d134ac1fc9b993156520ea826b8aa89a962d4491b4b8d7e08738e1da9b07aa39bf4a7ef0b1c210728cd52"
        );
    }

    #[test]
    fn md5_matches_known_vectors() {
        assert_eq!(md5_hex("123456"), "e10adc3949ba59abbe56e057f20f883e");
        assert_eq!(md5_hex("password"), "5f4dcc3b5aa765d61d8327deb882cf99");
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn form_body_has_two_urlencoded_fields() {
        let form = encrypt_payload(&serde_json::json!({ "s": "a b", "limit": 20 }))
            .expect("weapi 加密应成功");
        let body = form.body();
        assert!(body.starts_with("params="));
        assert_eq!(body.matches("&encSecKey=").count(), 1);
        assert!(!body.contains(' '));
        assert!(!body.contains('\n'));
    }
}
