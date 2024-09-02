use lazy_static::lazy_static;
use std::env;
use std::time::Duration;

#[derive(Debug,)]
pub struct TokenConfig {
    pub jwt_refresh_token_private_key: String,
    pub jwt_access_token_private_key: String,
    pub access_token_duration: Duration,
    pub refresh_token_duration: Duration,
}

#[derive(Debug, Clone,)]
pub struct AwsS3 {
    pub bucket_name: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub host_url: String,
    pub cdn_host_url: String,
    pub default_avatars_folder_path: String,
}

#[derive(Debug, Clone,)]
pub struct Crafter {
    pub url: String,
}

impl Default for Crafter {
    fn default() -> Self {
        Self {
            url: env::var("DEVLOG_CRAFTER_URL",).unwrap_or("http://localhost:8025".to_owned(),),
        }
    }
}

#[derive(Debug, Clone,)]
pub struct Smtp {
    pub host: String,
    pub username: String,
    pub password: String,
    pub port: i32,
}

impl Default for Smtp {
    fn default() -> Self {
        Self {
            host: env::var("DEVLOG_SMTP_HOST",).unwrap_or("127.0.0.1".to_owned(),),
            port: env::var("DEVLOG_SMTP_PORT",)
                .map(|it| it.parse().expect("DEVLOG_SMTP_PORT must be a number",),)
                .unwrap_or(8025,),
            username: env::var("DEVLOG_SMTP_USERNAME",).unwrap_or("root".to_owned(),),
            password: env::var("DEVLOG_SMTP_PASSWORD",).unwrap_or("root".to_owned(),),
        }
    }
}

impl Default for AwsS3 {
    fn default() -> Self {
        Self {
            host_url: env::var("AWS_ENDPOINT_URL",).expect("AWS_ENDPOINT_URL must be defined",),
            cdn_host_url: env::var("AWS_CDN_ENDPOINT_URL",)
                .unwrap_or(env::var("AWS_ENDPOINT_URL",).expect("AWS_ENDPOINT_URL must be defined",),),
            bucket_name: env::var("AWS_S3_BUCKET_NAME",).expect("AWS_S3_BUCKET_NAME must be defined",),
            access_key: env::var("AWS_ACCESS_KEY_ID",).expect("AWS_ACCESS_KEY_ID must be defined",),
            secret_key: env::var("AWS_SECRET_ACCESS_KEY",).expect("AWS_SECRET_ACCESS_KEY must be defined",),
            region: env::var("AWS_S3_REGION",).expect("AWS_S3_REGION must be defined",),
            default_avatars_folder_path: env::var("DEVLOG_DEFAULT_AVATARS_S3_FOLDER_PATH",)
                .unwrap_or(String::from("users/avatars/defaults/",),),
        }
    }
}

impl Default for TokenConfig {
    fn default() -> Self {
        let refresh_token_duration = Duration::from_millis(
            env::var("DEVLOG_REFRESH_TOKEN_LIFETIME_IN_MS",)
                .map(|env_var| env_var.parse().expect("The DEVLOG_REFRESH_TOKEN_LIFETIME_IN_MS must be number",),)
                .unwrap_or(86400000,),
        ); // 1 day
        Self {
            jwt_refresh_token_private_key: env::var("DEVLOG_REFRESH_TOKEN_PRIVATE_KEY",)
                .unwrap_or("this_is_unsafe_keythis_is_unsafe_keythis_is_unsafe_key".to_owned(),),
            jwt_access_token_private_key: env::var("DEVLOG_ACCESS_TOKEN_PRIVATE_KEY",)
                .unwrap_or("this_is_unsafe_keythis_is_unsafe_keythis_is_unsafe_key".to_owned(),),
            access_token_duration: Duration::from_millis(
                env::var("DEVLOG_ACCESS_TOKEN_LIFETIME_IN_MS",)
                    .map(|env_var| env_var.parse().expect("The DEVLOG_ACCESS_TOKEN_LIFETIME_IN_MS must be number",),)
                    .unwrap_or(1000 * 60 * 60 * 24 * 256,),
            ),
            refresh_token_duration,
        }
    }
}

#[derive(Debug, Default,)]
pub struct Config {
    pub token: TokenConfig,
    pub aws_s3: AwsS3,
    pub smtp: Smtp,
    pub crafter: Crafter,
}

lazy_static! {
    pub static ref CONFIGS: Config = Default::default();
}
