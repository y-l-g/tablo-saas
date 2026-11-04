//! The public website, outside every panel.

use topcoat::{
    Result,
    router::page,
    view::{View, view},
};

/// `GET /`.
#[page("/")]
async fn home() -> Result<impl View> {
    Ok(view! { <h1>"It works."</h1> })
}
