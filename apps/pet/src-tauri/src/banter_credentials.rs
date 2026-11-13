use code_musume_contracts::BanterConfig;
use sha2::{Digest, Sha256};

pub struct Credentials {
    mode: String,
    #[cfg(feature = "e2e")]
    test_keys: std::collections::BTreeMap<String, String>,
}
impl Credentials {
    pub fn new(mode: &str) -> Self {
        Self {
            mode: mode.into(),
            #[cfg(feature = "e2e")]
            test_keys: Default::default(),
        }
    }
    fn target(&self, c: &BanterConfig) -> String {
        let endpoint = super::banter_api::endpoint(c)
            .map(|u| u.to_string())
            .unwrap_or_default();
        format!(
            "CodeMusume/Banter/{}/{:x}",
            self.mode,
            Sha256::digest(endpoint)
        )
    }
    pub fn read(&self, c: &BanterConfig) -> Result<Option<String>, String> {
        let target = self.target(c);
        #[cfg(feature = "e2e")]
        return Ok(self.test_keys.get(&target).cloned());
        #[cfg(not(feature = "e2e"))]
        system::read(&target)
    }
    pub fn store(&mut self, c: &BanterConfig, value: Option<&str>) -> Result<(), String> {
        let target = self.target(c);
        #[cfg(feature = "e2e")]
        {
            if let Some(value) = value {
                self.test_keys.insert(target, value.into());
            } else {
                self.test_keys.remove(&target);
            }
            Ok(())
        }
        #[cfg(not(feature = "e2e"))]
        system::store(&target, value)
    }
}

#[cfg(all(windows, not(feature = "e2e")))]
mod system {
    use std::{io, ptr, slice};
    use windows_sys::Win32::Security::Credentials::*;

    const MISSING: Option<i32> = Some(1168);
    const ERROR: &str = "无法访问 Windows 凭据管理器";
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    pub fn read(target: &str) -> Result<Option<String>, String> {
        let target = wide(target);
        let mut pointer = ptr::null_mut();
        // Windows owns the returned buffer until CredFree.
        unsafe {
            if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) == 0 {
                return if io::Error::last_os_error().raw_os_error() == MISSING {
                    Ok(None)
                } else {
                    Err(ERROR.into())
                };
            }
            let entry = &*pointer;
            let result = if entry.CredentialBlobSize == 0 {
                Ok(None)
            } else {
                std::str::from_utf8(slice::from_raw_parts(
                    entry.CredentialBlob,
                    entry.CredentialBlobSize as usize,
                ))
                .map(|s| Some(s.to_owned()))
                .map_err(|_| ERROR.into())
            };
            CredFree(pointer.cast());
            result
        }
    }
    pub fn store(target: &str, value: Option<&str>) -> Result<(), String> {
        let mut target = wide(target);
        let result = unsafe {
            if let Some(key) = value {
                let mut key = key.as_bytes().to_vec();
                let entry = CREDENTIALW {
                    Type: CRED_TYPE_GENERIC,
                    TargetName: target.as_mut_ptr(),
                    CredentialBlobSize: key.len() as u32,
                    CredentialBlob: key.as_mut_ptr(),
                    Persist: CRED_PERSIST_LOCAL_MACHINE,
                    ..Default::default()
                };
                CredWriteW(&entry, 0)
            } else {
                CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0)
            }
        };
        if result != 0 || value.is_none() && io::Error::last_os_error().raw_os_error() == MISSING {
            Ok(())
        } else {
            Err(ERROR.into())
        }
    }
}

#[cfg(all(not(windows), not(feature = "e2e")))]
mod system {
    pub fn read(_: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    pub fn store(_: &str, _: Option<&str>) -> Result<(), String> {
        Err("此平台不支持 Windows 凭据管理器".into())
    }
}
