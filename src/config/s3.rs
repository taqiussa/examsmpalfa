use aws_sdk_s3::{config::Builder as S3ConfigBuilder, Client};
use aws_config::BehaviorVersion;
use aws_types::region::Region;
use std::env;

#[derive(Clone)]
pub struct S3State {
    pub client: Client,
    pub bucket: String,
    pub public_base_url: String,
}

pub async fn build_from_env() -> Option<S3State> {
    let bucket = env::var("AWS_BUCKET").ok()?;
    let region_value = env::var("AWS_DEFAULT_REGION").unwrap_or_else(|_| "us-east-1".into());
    let endpoint = env::var("AWS_ENDPOINT").ok();
    let public_base_url = env::var("AWS_URL").ok().or_else(|| {
        endpoint
            .as_ref()
            .map(|e| format!("{}/{}", e.trim_end_matches('/'), bucket))
    }).unwrap_or_else(|| {
        format!("https://{}.s3.{}.amazonaws.com", bucket, region_value)
    });

    let region = Region::new(region_value);
    let mut config_loader = aws_config::defaults(BehaviorVersion::latest()).region(region.clone());
    if let Some(ref endpoint_url) = endpoint {
        config_loader = config_loader.endpoint_url(endpoint_url);
    }
    let shared_config = config_loader.load().await;

    let mut s3_config_builder = S3ConfigBuilder::from(&shared_config);
    if let Some(endpoint_url) = endpoint {
        s3_config_builder = s3_config_builder.endpoint_url(endpoint_url);
    }

    let use_path_style = env::var("AWS_USE_PATH_STYLE_ENDPOINT")
        .ok()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if use_path_style {
        s3_config_builder = s3_config_builder.force_path_style(true);
    }

    let client = Client::from_conf(s3_config_builder.build());

    Some(S3State {
        client,
        bucket,
        public_base_url,
    })
}
