use std::path::Path;
use std::sync::Arc;

use common::profile::pics_profile::PicsProfile;
use common::profile::validation::Validate;
use poem::web::{Data, Json, Multipart, Path as PoemPath};
use poem::{IntoResponse, Route, get, handler, post};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::job::ValidationErrorsResponse;
use crate::services::job_service::JobService;

/// Provenance of a profile entry.
///
/// `Seed` entries live under `<data_dir>/profiles/` (and the bundled
/// `template/profile.json`) and are read-only at the deployment layer.
/// `Working` entries live under `<data_dir>/working/` and are written
/// by the save endpoint.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ProfileSource {
    Seed,
    Working,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ProfileListItem {
    pub name: String,
    pub filename: String,
    pub last_modified: Option<u64>,
    pub source: ProfileSource,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveProfileRequest {
    pub name: String,
    /// Profile JSON data. Accepts both "profile" and "data" field names
    /// for backward compatibility with the existing frontend.
    ///
    /// The runtime type stays `serde_json::Value` (preserves the existing
    /// permissive deserialize behavior); the OpenAPI schema reports the
    /// stricter `PicsProfile` shape via `#[schema(value_type = PicsProfile)]`.
    #[serde(alias = "data")]
    #[schema(value_type = PicsProfile)]
    pub profile: serde_json::Value,
}

/// Acknowledgement payload returned by the mutation endpoints.
/// Defined here so the schema can be referenced by `#[utoipa::path]` even
/// though the handlers still return `serde_json::Value` for back-compat.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProfileMutationAck {
    pub status: String,
    pub name: String,
}

/// Reject profile names that could escape the profiles directory.
fn validate_profile_name(name: &str) -> poem::Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
    {
        return Err(poem::Error::from_string(
            "Invalid profile name",
            poem::http::StatusCode::BAD_REQUEST,
        ));
    }
    Ok(())
}

async fn get_modified_time(path: &Path) -> Option<u64> {
    tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// List all `*.json` profile entries in `dir`. Returns each entry's stem name,
/// the supplied path-relative filename, and its last-modified timestamp.
/// Returns an empty `Vec` if the directory does not exist (callers aggregate).
async fn read_profile_dir(
    dir: &Path,
    rel_prefix: &str,
) -> poem::Result<Vec<(String, String, Option<u64>)>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = tokio::fs::read_dir(dir).await.map_err(|e| {
        poem::Error::from_string(
            format!("Failed to read {rel_prefix} directory: {e}"),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let mut out = vec![];
    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        poem::Error::from_string(
            format!("Failed to read directory entry: {e}"),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })? {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let filename = format!("{rel_prefix}/{name}.json");
            let mtime = get_modified_time(&path).await;
            out.push((name, filename, mtime));
        }
    }
    Ok(out)
}

#[utoipa::path(
    get,
    path = "/api/profiles",
    tag = "profiles",
    responses(
        (status = 200, description = "List saved profiles plus the bundled template", body = Vec<ProfileListItem>),
        (status = 500, description = "Failed to read profiles directory")
    )
)]
/// List all profiles visible to the API.
///
/// Aggregates entries from three locations:
///   1. `<data_dir>/working/` — user saves, reported as `source: working`.
///   2. `<data_dir>/profiles/` — curated read-only seeds, reported as
///      `source: seed`. If a name appears in both `working/` and `profiles/`,
///      `working/` shadows `profiles/` (the user's save wins). This mirrors
///      `get_profile`'s resolution order so the listing matches what the
///      load endpoint will actually return.
///   3. `<data_dir>/template/profile.json` — the bundled template entry,
///      reported as `source: seed`.
///
/// The result is sorted by `last_modified` descending.
#[handler]
pub async fn list_profiles(
    service: Data<&Arc<JobService>>,
) -> poem::Result<Json<Vec<ProfileListItem>>> {
    let data_dir = service.data_dir();
    let working_dir = Path::new(data_dir).join("working");
    let profiles_dir = Path::new(data_dir).join("profiles");
    let template_dir = Path::new(data_dir).join("template");

    let mut profiles = vec![];

    // 1. Working profiles first — they shadow seeds on name collision.
    let working_entries = read_profile_dir(&working_dir, "working").await?;
    for (name, filename, mtime) in working_entries {
        profiles.push(ProfileListItem {
            name,
            filename,
            last_modified: mtime,
            source: ProfileSource::Working,
        });
    }

    // 2. Seed profiles — only include names not already present from working.
    let seed_entries = read_profile_dir(&profiles_dir, "profiles").await?;
    for (name, filename, mtime) in seed_entries {
        if profiles.iter().any(|p| p.name == name) {
            continue;
        }
        profiles.push(ProfileListItem {
            name,
            filename,
            last_modified: mtime,
            source: ProfileSource::Seed,
        });
    }

    // 3. Bundled template entry.
    let template_path = template_dir.join("profile.json");
    if template_path.exists() {
        profiles.push(ProfileListItem {
            name: "template".to_string(),
            filename: "template/profile.json".to_string(),
            last_modified: get_modified_time(&template_path).await,
            source: ProfileSource::Seed,
        });
    }

    // Sort by last_modified descending (most recent first)
    profiles.sort_by_key(|profile_list_item| std::cmp::Reverse(profile_list_item.last_modified));

    Ok(Json(profiles))
}

#[utoipa::path(
    get,
    path = "/api/profiles/{name}",
    tag = "profiles",
    params(
        ("name" = String, Path, description = "Profile name (without .json extension)")
    ),
    responses(
        (status = 200, description = "Profile JSON document", body = PicsProfile),
        (status = 400, description = "Invalid profile name (path traversal rejected)"),
        (status = 404, description = "Profile not found"),
        (status = 500, description = "Failed to read or parse profile")
    )
)]
/// Resolve and return a profile by name.
///
/// Resolution order: special `name == "template"` maps to
/// `<data_dir>/template/profile.json`; otherwise look in `<data_dir>/working/`,
/// then `<data_dir>/profiles/`, then `<data_dir>/template/{name}.json` as a
/// legacy fallback. 404 if none match. Working shadows seed so a saved working
/// copy always wins over the same-named seed.
#[handler]
pub async fn get_profile(
    service: Data<&Arc<JobService>>,
    PoemPath(name): PoemPath<String>,
) -> poem::Result<Json<serde_json::Value>> {
    validate_profile_name(&name)?;

    let data_dir = service.data_dir();

    // Special case: `list_profiles` advertises the bundled template as
    // `{name: "template", filename: "template/profile.json"}`. Honour that
    // alias by mapping `name == "template"` to template/profile.json.
    let path = if name == "template" {
        let template_profile = Path::new(data_dir).join("template").join("profile.json");
        if !template_profile.exists() {
            return Err(poem::Error::from_string(
                "Profile 'template' not found".to_string(),
                poem::http::StatusCode::NOT_FOUND,
            ));
        }
        template_profile
    } else {
        let working_path = Path::new(data_dir)
            .join("working")
            .join(format!("{name}.json"));
        let seed_path = Path::new(data_dir)
            .join("profiles")
            .join(format!("{name}.json"));
        let template_fallback = Path::new(data_dir)
            .join("template")
            .join(format!("{name}.json"));

        if working_path.exists() {
            working_path
        } else if seed_path.exists() {
            seed_path
        } else if template_fallback.exists() {
            template_fallback
        } else {
            return Err(poem::Error::from_string(
                format!("Profile '{name}' not found"),
                poem::http::StatusCode::NOT_FOUND,
            ));
        }
    };

    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        poem::Error::from_string(
            format!("Failed to read profile: {e}"),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        poem::Error::from_string(
            format!("Failed to parse profile JSON: {e}"),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(json))
}

#[utoipa::path(
    post,
    path = "/api/profiles/validate",
    tag = "profiles",
    request_body = PicsProfile,
    operation_id = "validateProfile",
    responses(
        (status = 200, description = "Profile is valid", body = String),
        (status = 400, description = "Invalid profile JSON or validation errors", body=ValidationErrorsResponse)
    )
)]
#[handler]
pub async fn validate_profile(body: Json<PicsProfile>) -> poem::Result<String> {
    match body.validate() {
        Ok(()) => Ok("No validation errors".to_string()),
        Err(errors) => Err(poem::Error::from_response(
            ValidationErrorsResponse { errors }.into_response(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/api/profiles",
    tag = "profiles",
    request_body = SaveProfileRequest,
    responses(
        (status = 200, description = "Profile saved", body = ProfileMutationAck),
        (status = 400, description = "Invalid profile name (path traversal rejected)"),
        (status = 500, description = "Failed to write profile to disk or working dir not mounted")
    )
)]
/// Save a profile under `<data_dir>/working/{name}.json`.
///
/// Always writes to `working/`. Never touches `profiles/` (those are
/// curated read-only seeds). If a save name collides with a seed, the
/// resulting working copy shadows the seed in `list_profiles` and
/// `get_profile`. The frontend is responsible for warning the user
/// before overwriting an existing working copy.
#[handler]
pub async fn save_profile(
    service: Data<&Arc<JobService>>,
    body: Json<SaveProfileRequest>,
) -> poem::Result<Json<serde_json::Value>> {
    validate_profile_name(&body.name)?;

    let data_dir = service.data_dir();
    let working_dir = Path::new(data_dir).join("working");

    // The working directory is provisioned by the deployment layer (compose
    // mount in Phase 1). If it's missing we treat it as a deployment bug
    // rather than silently creating it on a read-only mount.
    if !working_dir.exists() {
        return Err(poem::Error::from_string(
            format!(
                "{} is not mounted; this deployment is misconfigured",
                working_dir.display()
            ),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    let path = working_dir.join(format!("{}.json", body.name));
    let json_str = serde_json::to_string_pretty(&body.profile).map_err(|e| {
        poem::Error::from_string(
            format!("Failed to serialize profile: {e}"),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    tokio::fs::write(&path, json_str).await.map_err(|e| {
        poem::Error::from_string(
            format!("Failed to write profile: {e}"),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(serde_json::json!({
        "status": "saved",
        "name": body.name,
    })))
}

#[utoipa::path(
    delete,
    path = "/api/profiles/{name}",
    tag = "profiles",
    params(
        ("name" = String, Path, description = "Profile name (without .json extension)")
    ),
    responses(
        (status = 200, description = "Profile deleted", body = ProfileMutationAck),
        (status = 400, description = "Invalid profile name or attempt to delete a seed"),
        (status = 404, description = "Profile not found"),
        (status = 500, description = "Failed to delete profile")
    )
)]
/// Delete a working profile.
///
/// Only working-directory entries can be deleted. If the name resolves
/// to `<data_dir>/working/{name}.json`, it is removed. If it would only
/// resolve to a seed (`<data_dir>/profiles/{name}.json`), the request
/// is rejected with 400 — seeds are immutable through the API. 404 if
/// neither location holds the file.
#[handler]
pub async fn delete_profile(
    service: Data<&Arc<JobService>>,
    PoemPath(name): PoemPath<String>,
) -> poem::Result<Json<serde_json::Value>> {
    validate_profile_name(&name)?;

    let data_dir = service.data_dir();
    let working_path = Path::new(data_dir)
        .join("working")
        .join(format!("{name}.json"));
    let seed_path = Path::new(data_dir)
        .join("profiles")
        .join(format!("{name}.json"));

    if working_path.exists() {
        tokio::fs::remove_file(&working_path).await.map_err(|e| {
            poem::Error::from_string(
                format!("Failed to delete profile: {e}"),
                poem::http::StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;
        Ok(Json(serde_json::json!({
            "status": "deleted",
            "name": name,
        })))
    } else if seed_path.exists() {
        // Chose 400 over 403: from the API's perspective the seed directory
        // simply isn't a delete target, so this is a malformed request rather
        // than an authorization failure. (No auth/identity layer here yet.)
        Err(poem::Error::from_string(
            format!("Profile '{name}' is a read-only seed and cannot be deleted"),
            poem::http::StatusCode::BAD_REQUEST,
        ))
    } else {
        Err(poem::Error::from_string(
            format!("Profile '{name}' not found"),
            poem::http::StatusCode::NOT_FOUND,
        ))
    }
}

/// Import an xlsx workbook and return the parsed profile as JSON.
///
/// Accepts a `multipart/form-data` request with a single file field named
/// `file`. The file must be a valid `.xlsx` workbook. The parsed profile
/// is returned directly without being saved to disk.
///
/// The upload is rejected with 400 when the file exceeds `MAX_XLSX_BYTES`.
/// Error messages are sanitized: no internal paths or stack traces are
/// returned to the caller.
#[utoipa::path(
    post,
    path = "/api/profiles/parse-xlsx",
    tag = "profiles",
    request_body(
        description = "The `.xlsx` file to parse (max 10 MB)",
        content_type = "multipart/form-data",
    ),
    responses(
        (status = 200, description = "parsed profile", body = PicsProfile),
        (status = 400, description = "Invalid file, size exceeded, or parse error")
    )
)]
#[handler]
pub async fn parse_xlsx(mut multipart: Multipart) -> poem::Result<Json<serde_json::Value>> {
    // 10 MB ceiling. Real MESA profiles are a few hundred KB at most; this
    // gives comfortable headroom while bounding memory allocation from
    // untrusted uploads.
    const MAX_XLSX_BYTES: usize = 10 * 1024 * 1024;

    let mut file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|_| {
        poem::Error::from_string(
            "Failed to read multipart field".to_string(),
            poem::http::StatusCode::BAD_REQUEST,
        )
    })? {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|_| {
                poem::Error::from_string(
                    "Failed to read file bytes".to_string(),
                    poem::http::StatusCode::BAD_REQUEST,
                )
            })?;
            if bytes.len() > MAX_XLSX_BYTES {
                return Err(poem::Error::from_string(
                    format!(
                        "File too large: {} bytes exceeds the {} byte limit",
                        bytes.len(),
                        MAX_XLSX_BYTES
                    ),
                    poem::http::StatusCode::BAD_REQUEST,
                ));
            }
            file_bytes = Some(bytes.to_vec());
            break;
        }
    }

    let bytes = file_bytes.ok_or_else(|| {
        poem::Error::from_string(
            "No file field found in multipart upload".to_string(),
            poem::http::StatusCode::BAD_REQUEST,
        )
    })?;

    let workbook = pics_validator::load_workbook_from_bytes(&bytes).map_err(|_| {
        poem::Error::from_string(
            "Failed to parse xlsx: the file is not a valid xlsx workbook".to_string(),
            poem::http::StatusCode::BAD_REQUEST,
        )
    })?;

    let profile = pics_validator::workbook_to_profile(workbook).map_err(|err| {
        // The workbook_to_profile error chain is safe to surface: it names
        // missing sheets and structural issues without referencing filesystem
        // paths (bytes are parsed in-memory, no temp file is involved).
        poem::Error::from_string(
            format!("Profile conversion failed: {err}"),
            poem::http::StatusCode::BAD_REQUEST,
        )
    })?;

    let profile_json = serde_json::to_value(profile).map_err(|err| {
        poem::Error::from_string(
            format!("Failed to serialize profile: {err}"),
            poem::http::StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    Ok(Json(profile_json))
}

pub fn routes() -> Route {
    Route::new()
        .at("/", get(list_profiles).post(save_profile))
        .at("/parse-xlsx", post(parse_xlsx))
        .at("/:name", get(get_profile).delete(delete_profile))
        .at("/validate", post(validate_profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use poem::EndpointExt;
    use poem::middleware::AddDataEndpoint;
    use poem::test::{TestClient, TestResponse};
    use std::fs;

    /// Create a temp directory with optional fixtures and return the path.
    ///
    /// `with_template`: if true, write `template/profile.json`.
    /// `seed_profiles`: written into `profiles/` (read-only seeds).
    /// `working_profiles`: written into `working/` (user saves).
    ///
    /// `working/` is always created (even if empty) so save_profile's
    /// "is the working dir mounted?" check passes in tests by default.
    fn setup_test_data_dir(
        with_template: bool,
        seed_profiles: &[(&str, &str)],
        working_profiles: &[(&str, &str)],
    ) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");

        if with_template {
            let template_dir = tmp.path().join("template");
            fs::create_dir_all(&template_dir).unwrap();
            fs::write(
                template_dir.join("profile.json"),
                r#"{"binary_outputs":{"points":[]},"binary_inputs":{"points":[]},"analog_outputs":{"points":[]},"analog_inputs":{"points":[]}}"#,
            )
            .unwrap();
        }

        if !seed_profiles.is_empty() {
            let profiles_dir = tmp.path().join("profiles");
            fs::create_dir_all(&profiles_dir).unwrap();
            for (name, content) in seed_profiles {
                fs::write(profiles_dir.join(format!("{name}.json")), content).unwrap();
            }
        }

        // Always provision working/ so save_profile's mount check passes.
        let working_dir = tmp.path().join("working");
        fs::create_dir_all(&working_dir).unwrap();
        for (name, content) in working_profiles {
            fs::write(working_dir.join(format!("{name}.json")), content).unwrap();
        }

        tmp
    }

    fn test_profiles_app(data_dir: &str) -> AddDataEndpoint<Route, Arc<JobService>> {
        let service = Arc::new(JobService::new(data_dir.to_string()));
        Route::new().nest("/", routes()).data(service)
    }

    /// Helper to deserialize the response body as a Vec<ProfileListItem>.
    async fn parse_profile_list(resp: TestResponse) -> Vec<ProfileListItem> {
        let body = resp.0.into_body().into_string().await.unwrap();
        serde_json::from_str(&body).unwrap()
    }

    /// Helper to deserialize the response body as a serde_json::Value.
    async fn parse_json(resp: TestResponse) -> serde_json::Value {
        let body = resp.0.into_body().into_string().await.unwrap();
        serde_json::from_str(&body).unwrap()
    }

    // --- list_profiles tests ---

    #[tokio::test]
    async fn test_list_profiles_returns_template() {
        let tmp = setup_test_data_dir(true, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();

        let items = parse_profile_list(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "template");
        assert_eq!(items[0].filename, "template/profile.json");
        assert_eq!(items[0].source, ProfileSource::Seed);
    }

    #[tokio::test]
    async fn test_list_profiles_empty_data_dir() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();

        let items = parse_profile_list(resp).await;
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn test_list_profiles_includes_working_profiles() {
        let profile_json = r#"{"binary_outputs":{"points":[]}}"#;
        let tmp = setup_test_data_dir(true, &[], &[("my_profile", profile_json)]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();

        let items = parse_profile_list(resp).await;
        assert_eq!(items.len(), 2);

        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"template"));
        assert!(names.contains(&"my_profile"));

        let mine = items.iter().find(|i| i.name == "my_profile").unwrap();
        assert_eq!(mine.source, ProfileSource::Working);
        assert_eq!(mine.filename, "working/my_profile.json");
    }

    #[tokio::test]
    async fn test_list_profiles_ignores_non_json_files() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        // Create a non-json file in working dir
        let working_dir = tmp.path().join("working");
        fs::write(working_dir.join("readme.txt"), "not a profile").unwrap();
        fs::write(
            working_dir.join("valid.json"),
            r#"{"binary_outputs":{"points":[]}}"#,
        )
        .unwrap();

        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();

        let items = parse_profile_list(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "valid");
        assert_eq!(items[0].source, ProfileSource::Working);
    }

    #[tokio::test]
    async fn test_list_profiles_aggregates_seeds_and_working() {
        let seed_json = r#"{"binary_outputs":{"points":[]}}"#;
        let working_json = r#"{"binary_inputs":{"points":[]}}"#;
        let tmp = setup_test_data_dir(
            false,
            &[("seed_one", seed_json), ("seed_two", seed_json)],
            &[("work_one", working_json)],
        );
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();

        let items = parse_profile_list(resp).await;
        assert_eq!(items.len(), 3);

        let by_name: std::collections::HashMap<&str, &ProfileListItem> =
            items.iter().map(|i| (i.name.as_str(), i)).collect();
        assert_eq!(by_name["seed_one"].source, ProfileSource::Seed);
        assert_eq!(by_name["seed_one"].filename, "profiles/seed_one.json");
        assert_eq!(by_name["seed_two"].source, ProfileSource::Seed);
        assert_eq!(by_name["work_one"].source, ProfileSource::Working);
        assert_eq!(by_name["work_one"].filename, "working/work_one.json");
    }

    #[tokio::test]
    async fn test_list_profiles_working_shadows_seed_on_name_collision() {
        // Same name in both dirs — working wins, only one entry returned.
        let seed_json = r#"{"from":"seed"}"#;
        let working_json = r#"{"from":"working"}"#;
        let tmp = setup_test_data_dir(false, &[("foo", seed_json)], &[("foo", working_json)]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/").send().await;
        resp.assert_status_is_ok();

        let items = parse_profile_list(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "foo");
        assert_eq!(items[0].source, ProfileSource::Working);
        assert_eq!(items[0].filename, "working/foo.json");
    }

    // --- get_profile tests ---

    #[tokio::test]
    async fn test_get_profile_template() {
        let tmp = setup_test_data_dir(true, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/profile").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert!(json.get("binary_outputs").is_some());
        assert!(json.get("binary_inputs").is_some());
        assert!(json.get("analog_outputs").is_some());
        assert!(json.get("analog_inputs").is_some());
    }

    #[tokio::test]
    async fn test_get_profile_working_profile() {
        let profile_content = r#"{"custom_field": "hello"}"#;
        let tmp = setup_test_data_dir(false, &[], &[("custom", profile_content)]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/custom").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert_eq!(json["custom_field"], "hello");
    }

    #[tokio::test]
    async fn test_get_profile_user_overrides_template() {
        // Resolution order: working > seed > template-fallback.
        // A working entry with the same name as the template fallback wins.
        let user_content = r#"{"source": "user"}"#;
        let tmp = setup_test_data_dir(true, &[], &[("profile", user_content)]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/profile").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert_eq!(json["source"], "user");
    }

    #[tokio::test]
    async fn test_get_profile_working_takes_precedence_over_seed() {
        // Same name in both dirs with different content — working wins.
        let seed_content = r#"{"source":"seed"}"#;
        let working_content = r#"{"source":"working"}"#;
        let tmp = setup_test_data_dir(false, &[("foo", seed_content)], &[("foo", working_content)]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/foo").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert_eq!(json["source"], "working");
    }

    #[tokio::test]
    async fn test_get_profile_falls_back_to_seed_when_working_missing() {
        // Only seed exists — get returns the seed content.
        let seed_content = r#"{"source":"seed","value":42}"#;
        let tmp = setup_test_data_dir(false, &[("only_seed", seed_content)], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/only_seed").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert_eq!(json["source"], "seed");
        assert_eq!(json["value"], 42);
    }

    #[tokio::test]
    async fn test_get_profile_not_found() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/nonexistent").send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    // --- save_profile tests ---

    #[tokio::test]
    async fn test_save_profile_creates_file() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "name": "new_profile",
                "profile": {
                    "binary_outputs": {"points": []},
                    "binary_inputs": {"points": []}
                }
            }))
            .send()
            .await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert_eq!(json["status"], "saved");
        assert_eq!(json["name"], "new_profile");

        // File lands in working/, never profiles/.
        let saved_path = tmp.path().join("working/new_profile.json");
        assert!(saved_path.exists());
        let profiles_path = tmp.path().join("profiles/new_profile.json");
        assert!(!profiles_path.exists());
    }

    #[tokio::test]
    async fn test_save_profile_writes_to_working_not_profiles() {
        // Even with an existing seed, save creates a sibling working copy
        // and leaves the seed dir untouched.
        let seed_content = r#"{"i":"am","a":"seed"}"#;
        let tmp = setup_test_data_dir(false, &[("full", seed_content)], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "name": "full",
                "profile": {"i":"am","a":"working_copy"}
            }))
            .send()
            .await;
        resp.assert_status_is_ok();

        // working/full.json exists.
        let working_path = tmp.path().join("working/full.json");
        assert!(working_path.exists());

        // profiles/full.json is byte-identical to before.
        let on_disk = fs::read_to_string(tmp.path().join("profiles/full.json")).unwrap();
        assert_eq!(on_disk, seed_content);
    }

    #[tokio::test]
    async fn test_save_then_get_round_trip() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let profile_data = serde_json::json!({
            "binary_outputs": {"points": [{"index": 0}]},
            "binary_inputs": {"points": []}
        });

        // Save
        let save_resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "name": "roundtrip",
                "profile": profile_data
            }))
            .send()
            .await;
        save_resp.assert_status_is_ok();

        // Save lands in working/.
        assert!(tmp.path().join("working/roundtrip.json").exists());

        // Get pulls it back (from working/).
        let get_resp: TestResponse = client.get("/roundtrip").send().await;
        get_resp.assert_status_is_ok();

        let json = parse_json(get_resp).await;
        let points = json["binary_outputs"]["points"].as_array().unwrap();
        assert_eq!(points.len(), 1);
    }

    // --- path traversal validation tests (Issue #157) ---

    #[tokio::test]
    async fn test_get_profile_rejects_path_traversal() {
        let tmp = setup_test_data_dir(true, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        // Bare `../` is normalized by the router and never reaches the handler (404).
        // The critical check is that names containing `..` are rejected at the handler level.
        let resp: TestResponse = client.get("/..%2Fetc%2Fpasswd").send().await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_profile_rejects_url_encoded_traversal() {
        let tmp = setup_test_data_dir(true, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/..%2F..%2Fetc%2Fpasswd").send().await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_save_profile_rejects_traversal_name() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "name": "../evil",
                "profile": {"test": true}
            }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_profile_allows_valid_name() {
        let profile_content = r#"{"valid": true}"#;
        let tmp = setup_test_data_dir(false, &[], &[("valid-name", profile_content)]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/valid-name").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert_eq!(json["valid"], true);
    }

    #[tokio::test]
    async fn test_get_profile_template_still_works() {
        let tmp = setup_test_data_dir(true, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/profile").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert!(json.get("binary_outputs").is_some());
    }

    #[tokio::test]
    async fn test_get_profile_by_name_template_resolves_special_case() {
        // Verifies the f53b5eee special case: GET /template loads
        // template/profile.json even though the listing's name doesn't match
        // the file's stem. The existing test_get_profile_template uses
        // GET /profile (the legacy template/{name}.json fallback path).
        let tmp = setup_test_data_dir(true, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.get("/template").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert!(json.get("binary_outputs").is_some());
        assert!(json.get("analog_inputs").is_some());
    }

    #[tokio::test]
    async fn test_save_then_list_includes_new_profile() {
        let tmp = setup_test_data_dir(true, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        // Save a new profile
        let save_resp: TestResponse = client
            .post("/")
            .body_json(&serde_json::json!({
                "name": "listed_profile",
                "profile": {"test": true}
            }))
            .send()
            .await;
        save_resp.assert_status_is_ok();

        // List should include it with source: working.
        let list_resp: TestResponse = client.get("/").send().await;
        list_resp.assert_status_is_ok();

        let items = parse_profile_list(list_resp).await;
        assert_eq!(items.len(), 2);

        let listed = items.iter().find(|i| i.name == "listed_profile").unwrap();
        assert_eq!(listed.source, ProfileSource::Working);
        assert_eq!(listed.filename, "working/listed_profile.json");
    }

    // --- delete_profile tests ---

    #[tokio::test]
    async fn test_delete_profile_removes_working() {
        let tmp = setup_test_data_dir(false, &[], &[("doomed", r#"{"x":1}"#)]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let path = tmp.path().join("working/doomed.json");
        assert!(path.exists());

        let resp: TestResponse = client.delete("/doomed").send().await;
        resp.assert_status_is_ok();

        let json = parse_json(resp).await;
        assert_eq!(json["status"], "deleted");
        assert_eq!(json["name"], "doomed");
        assert!(!path.exists());

        // List no longer includes it.
        let list_resp: TestResponse = client.get("/").send().await;
        let items = parse_profile_list(list_resp).await;
        assert!(items.iter().all(|i| i.name != "doomed"));
    }

    #[tokio::test]
    async fn test_delete_profile_rejects_seed() {
        // A seed-only entry cannot be deleted. The seed file remains on disk.
        let tmp = setup_test_data_dir(false, &[("immortal", r#"{"y":2}"#)], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.delete("/immortal").send().await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);

        // Seed file untouched.
        assert!(tmp.path().join("profiles/immortal.json").exists());
    }

    #[tokio::test]
    async fn test_delete_profile_not_found() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client.delete("/ghost").send().await;
        resp.assert_status(poem::http::StatusCode::NOT_FOUND);
    }

    // --- parse_xlsx tests ---

    /// Build a multipart form body that wraps `file_bytes` under the field
    /// name "file", matching the shape expected by `parse_xlsx`.
    fn xlsx_form(file_bytes: Vec<u8>) -> poem::test::TestForm {
        poem::test::TestForm::new().field(
            poem::test::TestFormField::bytes(file_bytes)
                .name("file")
                .filename("profile.xlsx")
                .content_type("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        )
    }

    /// Resolve a path relative to the workspace root from the web_server
    /// package directory (three levels up).
    fn workspace_path(rel: &str) -> std::path::PathBuf {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("workspace root is three directories above CARGO_MANIFEST_DIR");
        workspace_root.join(rel)
    }

    /// Helper: POST an xlsx file and return the parsed JSON value.
    async fn post_xlsx(
        client: &TestClient<impl poem::Endpoint>,
        xlsx_path: &str,
    ) -> serde_json::Value {
        let bytes = fs::read(workspace_path(xlsx_path))
            .unwrap_or_else(|e| panic!("failed to read {xlsx_path}: {e}"));
        let resp: TestResponse = client
            .post("/parse-xlsx")
            .multipart(xlsx_form(bytes))
            .send()
            .await;
        resp.assert_status_is_ok();
        parse_json(resp).await
    }

    #[tokio::test]
    async fn test_parse_xlsx_full_profile_returns_valid_profile() {
        // POST data/profiles/full.xlsx and assert that the returned profile
        // has the expected top-level structure and a non-empty BO point list.
        // This also verifies that workbook_to_profile is reused (same output
        // shape as load_xlsx_profile from pics_validator integration tests).
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let json = post_xlsx(&client, "data/profiles/full.xlsx").await;

        // Top-level sections present (the profile schema uses uppercase keys).
        assert!(json.get("BO").is_some(), "expected BO section");
        assert!(json.get("BI").is_some(), "expected BI section");
        assert!(json.get("AO").is_some(), "expected AO section");
        assert!(json.get("AI").is_some(), "expected AI section");
        assert!(json.get("Key").is_some(), "expected Key section");

        // BO has at least one point (semantic value check, not just non-crash).
        let bo_points = json["BO"]["points"]
            .as_array()
            .expect("BO.points must be an array");
        assert!(
            !bo_points.is_empty(),
            "full.xlsx must contain at least one BO point"
        );
        // First BO point has point_index 0 (matches the loader invariant).
        assert_eq!(
            bo_points[0]["point_index"], 0,
            "first BO point_index must be 0"
        );
    }

    #[tokio::test]
    async fn test_parse_xlsx_mandatory_1815_profile() {
        // mandatory_1815.xlsx is one of the curated seeds; verify the endpoint
        // returns a parseable profile with at least some points.
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let json = post_xlsx(&client, "data/profiles/mandatory_1815.xlsx").await;

        assert!(json.get("BO").is_some());
        // Engineering value check: Key.max_points must be a positive integer
        // because every valid MESA profile sets this in the Key sheet.
        let max_points = json["Key"]["max_points"]
            .as_u64()
            .expect("Key.max_points must be a non-negative integer");
        assert!(
            max_points > 0,
            "Key.max_points must be > 0 for a real profile"
        );
    }

    #[tokio::test]
    async fn test_parse_xlsx_mandatory_1547_profile() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let json = post_xlsx(&client, "data/profiles/mandatory_1547.xlsx").await;

        assert!(json.get("BO").is_some());
        let max_points = json["Key"]["max_points"]
            .as_u64()
            .expect("Key.max_points must be a non-negative integer");
        assert!(
            max_points > 0,
            "Key.max_points must be > 0 for a real profile"
        );
    }

    #[tokio::test]
    async fn test_parse_xlsx_minimal_1547_profile() {
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let json = post_xlsx(&client, "data/profiles/minimal_1547.xlsx").await;

        assert!(json.get("BO").is_some());
        let max_points = json["Key"]["max_points"]
            .as_u64()
            .expect("Key.max_points must be a non-negative integer");
        assert!(
            max_points > 0,
            "Key.max_points must be > 0 for a real profile"
        );
    }

    #[tokio::test]
    async fn test_parse_xlsx_rejects_non_xlsx_body() {
        // POST random bytes that are not a valid xlsx workbook.
        // The endpoint must return 400 with a safe message (no paths, no
        // internal details beyond "not a valid xlsx workbook").
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let garbage = b"this is not an xlsx file".to_vec();
        let resp: TestResponse = client
            .post("/parse-xlsx")
            .multipart(xlsx_form(garbage))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);

        // The response body must not contain any filesystem path segments.
        let body = resp.0.into_body().into_string().await.unwrap();
        assert!(
            !body.contains("/home"),
            "error message must not leak fs paths"
        );
        assert!(
            !body.contains("\\\\"),
            "error message must not leak Windows paths"
        );
    }

    #[tokio::test]
    async fn test_parse_xlsx_rejects_missing_file_field() {
        // POST a multipart body that has no "file" field.
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        let resp: TestResponse = client
            .post("/parse-xlsx")
            .multipart(poem::test::TestForm::new().text("not_file", "hello"))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_parse_xlsx_rejects_oversized_file() {
        // A payload exceeding MAX_XLSX_BYTES (10 MiB) must be rejected with 400.
        let tmp = setup_test_data_dir(false, &[], &[]);
        let app = test_profiles_app(tmp.path().to_str().unwrap());
        let client = TestClient::new(app);

        // 10 MiB + 1 byte triggers the size guard.
        let oversized = vec![0u8; 10 * 1024 * 1024 + 1];
        let resp: TestResponse = client
            .post("/parse-xlsx")
            .multipart(xlsx_form(oversized))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::BAD_REQUEST);

        let body = resp.0.into_body().into_string().await.unwrap();
        assert!(
            body.contains("too large") || body.contains("limit"),
            "error message should mention size limit, got: {body}"
        );
    }
}
