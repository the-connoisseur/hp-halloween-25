#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::SqliteConnection;
    use dotenvy::dotenv;
    use hp_halloween_25::app::*;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use std::env;

    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env.");
    let _ = env::var("ADMIN_PASSWORD").expect("ADMIN_PASSWORD must be set in .env.");

    let manager = ConnectionManager::<SqliteConnection>::new(&database_url);
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(App);

    let leptos_options_clone = leptos_options.clone();
    let app = Router::new()
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            // Provide pool for server functions.
            move || provide_context(pool.clone()),
            // Use App for main routes.
            move || shell(leptos_options_clone.clone()),
        )
        // Use shell for fallback.
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options.clone());

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
