use crate::error::Result;
use crate::models::DirectoryService;

static SERVICE_PROVIDER_WELLKNOWN_URL: &str =
    "https://gist.githubusercontent.com/sven-hash/0e388d1cf305ffe94c9076659c2595eb/raw/service-providers.json";

#[tauri::command]
pub async fn get_services() -> Result<Vec<DirectoryService>> {
    let res = reqwest::get(SERVICE_PROVIDER_WELLKNOWN_URL)
        .await?
        .json::<Vec<DirectoryService>>()
        .await?;
    Ok(res)
}
