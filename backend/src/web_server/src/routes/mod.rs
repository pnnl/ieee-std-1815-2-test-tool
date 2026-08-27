pub mod curve_types;
pub mod enums;
pub mod health;
pub mod jobs;
pub mod profiles;
pub mod scenarios;

use poem::Route;

pub fn api_routes() -> Route {
    Route::new()
        .nest("/health", health::routes())
        .nest("/jobs", jobs::routes())
        .nest("/profiles", profiles::routes())
        .nest("/enums", enums::routes())
        .nest("/scenarios", scenarios::routes())
}
