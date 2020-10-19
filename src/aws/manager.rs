use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Lines};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Error};
use chrono::prelude::*;
use regex::Regex;
use rusoto_core::region::Region;
use serde::{Deserialize, Serialize};
use warp::reject;
use warp::Rejection;

use crate::error::ErrorWrapper;
use crate::extract_rejection;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub aws_use_default_credentials: bool,
    pub region: Option<String>,
    pub aws_sts_profile: Option<String>,
    pub aws_temp_access_key_id: Option<String>,
    pub aws_temp_secret_access_key: Option<String>,
    pub aws_session_token: Option<String>,
    pub aws_session_expiration: Option<DateTime<FixedOffset>>,
}

// TODO there is a LOT of work to populate this manager file, mostly around removing prompts and returning errors instead with
impl Config {
    pub fn init() -> Result<Config, Error> {
        let mut config: Config = Config {
            region: Some(Region::EuWest1.name().to_owned()),
            aws_use_default_credentials: false,
            ..Default::default()
        };

        config.aws_use_default_credentials = true;

        Ok(config)
    }

    pub fn load() -> Result<Config, Error> {
        let config_path = build_config_path()?;

        let mut config: Config;

        if config_path.exists() {
            let data = read_config_file(&config_path)?;
            config =
                serde_json::from_str(&data).with_context(|| "Invalid json in awsManager.json")?;
        } else {
            config = Self::init()?
        }

        if config.aws_use_default_credentials {
            set_default_aws_credentials(&mut config)?;
        }

        if !config_path.exists() {
            config.persist()?
        }

        Ok(config)
    }

    pub fn persist(&self) -> Result<(), Error> {
        let config_path = build_config_path()?;
        let file = File::create(config_path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }

    pub fn is_token_valid(&self) -> bool {
        if self.aws_session_token.is_none() {
            return false;
        }

        if self.aws_sts_profile.is_some() {
            // rely completely on the token from .aws/credentials
            return true;
        }

        match self.aws_session_expiration {
            Some(x) => Utc::now().timestamp_millis() < x.timestamp_millis(),
            None => false,
        }
    }
}

pub async fn setup_default_manager() -> Result<impl warp::Reply, Rejection> {
    debug!("Setting up default manager");
    extract_rejection!(Config::load())?;

    let reply_builder = warp::reply::reply();
    Ok(warp::reply::with_status(
        reply_builder,
        warp::http::StatusCode::CREATED,
    ))
}

fn set_default_aws_credentials(cfg: &mut Config) -> Result<(), Error> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Missing home directory"))?;
    debug!("Home directory found {:?}", home_dir);
    let file_path = build_credentials_path(home_dir)?;
    debug!("Aws file path found {:?}", file_path);
    let mut lines = read_lines(&file_path)?;

    let mut aws_access_key: Option<String> = None;
    let mut aws_secret_key: Option<String> = None;

    let mut profiles_cfg_map: HashMap<String, HashMap<String, Option<String>>> = HashMap::new();
    let mut profile_cfg: HashMap<String, Option<String>> = HashMap::new();

    let kv_regex = build_kv_regex();
    let profile_regex = build_profile_regex();
    let mut current_profile: Option<String> = None;
    loop {
        let line_opt = lines.next();

        if let Some(line_res) = line_opt {
            let raw_line = line_res?;
            let line = raw_line.trim();
            debug!("Parsing config line {}", &line);
            let mut caps = profile_regex.captures(&line);
            if let Some(profile_cap) = caps {
                if let Some(profile) = current_profile {
                    debug!("Added profile {} cfg {:?}", &profile, &profile_cfg);
                    profiles_cfg_map.insert(profile, profile_cfg);
                    profile_cfg = HashMap::new();
                }
                current_profile = Some(profile_cap.get(1).unwrap().as_str().to_owned());
                continue;
            }
            caps = kv_regex.captures(&line);
            if let Some(kv_cap) = caps {
                let k = kv_cap.get(1).unwrap().as_str().to_owned();
                let v = match kv_cap.get(2) {
                    Some(v) => Some(v.as_str().to_owned()),
                    None => None,
                };
                profile_cfg.insert(k, v);
            }
        } else {
            if let Some(profile) = current_profile {
                debug!("Added profile {} cfg {:?}", &profile, &profile_cfg);
                profiles_cfg_map.insert(profile, profile_cfg);
            }
            debug!("No more line");
            break;
        }
    }
    if let Some(default_profile_cfg) = profiles_cfg_map.get("default") {
        if let Some(aws_access_key_opt) = default_profile_cfg.get("aws_access_key_id") {
            aws_access_key = aws_access_key_opt.clone();
        }
        if let Some(aws_secret_key_opt) = default_profile_cfg.get("aws_secret_access_key") {
            aws_secret_key = aws_secret_key_opt.clone();
        }
    }
    for (profile_name, profile_cfg) in &profiles_cfg_map {
        debug!("Parsing profile {}", profile_name);
        if let Some(aws_session_token_opt) = profile_cfg.get("aws_session_token") {
            if aws_session_token_opt.is_some() {
                debug!("Found aws_session_token for profile {}", &profile_name);
                cfg.aws_sts_profile = Some(profile_name.clone());
                cfg.aws_session_token = aws_session_token_opt.clone();
            }
            if let Some(aws_access_key) =
                profile_cfg.get("aws_access_key_id").and_then(|v| v.clone())
            {
                debug!("Found aws_temp_access_key_id for profile {}", &profile_name);
                cfg.aws_temp_access_key_id = Some(aws_access_key);
            }
            if let Some(aws_secret_key) = profile_cfg
                .get("aws_secret_access_key")
                .and_then(|v| v.clone())
            {
                debug!("Found aws_secret_access_key for profile {}", &profile_name);
                cfg.aws_temp_secret_access_key = Some(aws_secret_key);
            }
            break;
        }
    }
    if aws_access_key.is_none() || aws_secret_key.is_none() {
        return Err(anyhow!(format!(
            "No aws credentials found in {:?}",
            file_path
        )));
    }

    cfg.aws_access_key_id = aws_access_key
        .ok_or_else(|| anyhow!(format!("No aws_access_key found in {:?}", file_path)))?;
    cfg.aws_secret_access_key = aws_secret_key
        .ok_or_else(|| anyhow!(format!("No aws_secrete_key found in {:?}", file_path)))?;

    Ok(())
}

fn read_lines(file_path: &PathBuf) -> Result<Lines<BufReader<File>>, Error> {
    let config_file = File::open(file_path.as_path())?;
    let reader = BufReader::new(config_file);
    let lines = reader.lines();
    Ok(lines)
}

fn build_profile_regex() -> Regex {
    Regex::new(r"\[([^]]+)]").unwrap()
}

fn build_kv_regex() -> Regex {
    Regex::new(r"\s*([^\s]+)\s*=\s*([^\s]*)\s*").unwrap()
}

fn build_credentials_path(home_dir: PathBuf) -> Result<PathBuf, Error> {
    let mut file_path = Path::new(home_dir.as_path()).join(".aws/credentials");
    if !file_path.exists() {
        let config_dir = dirs::config_dir().ok_or_else(|| anyhow!("Missing config directory"))?;
        file_path = Path::new(config_dir.as_path()).join(".aws/credentials");
        if !file_path.exists() {
            return Err(anyhow!("No aws credentials configuration found"));
        }
    }
    Ok(file_path)
}

fn build_config_path() -> Result<PathBuf, Error> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Missing home directory"))?;
    let config_path = Path::new(home_dir.as_path()).join(".awsManager.json");
    Ok(config_path)
}

fn read_config_file(config_path: &PathBuf) -> Result<String, Error> {
    let mut config_file =
        File::open(&config_path).with_context(|| format!("could not read {:?}", config_path))?;
    let mut data = String::new();
    config_file.read_to_string(&mut data)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_init() {
        let config = Config::init().unwrap();
        assert_eq!(format!("{:#?}", config), "")
    }

    #[test]
    fn test_config_load() {
        let config = Config::load().unwrap();
        assert_eq!(format!("{:#?}", config), "")
    }
}
