use leptos::ev::SubmitEvent;
use leptos::logging::log;
use leptos::prelude::*;
use leptos::server_fn::error::NoCustomError;
use leptos::task::spawn_local;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_navigate,
    path, NavigateOptions,
};
use rand::prelude::*;
use rand::rng;
use std::collections::HashMap;
use std::env;
#[cfg(feature = "hydrate")]
use wasm_bindgen::JsCast;

use crate::model::{Guest, House, PointAwardLog};
#[cfg(feature = "ssr")]
use crate::{
    award_points_to_guest, award_points_to_house, create_admin_session, get_all_active_guests,
    get_all_houses, get_all_point_awards, get_all_unregistered_guests, get_guest_by_token,
    get_guest_token, register_guest, reregister_guest, unregister_guest, validate_admin_token,
};

#[cfg(feature = "ssr")]
use diesel::r2d2::{ConnectionManager, Pool};
#[cfg(feature = "ssr")]
use diesel::SqliteConnection;
#[cfg(feature = "ssr")]
pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

#[server(GetHouses)]
pub async fn get_houses() -> Result<Vec<House>, ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        get_all_houses(&mut conn).map_err(|e| ServerFnError::ServerError(e.to_string()))
    })
    .await;
    match result {
        Ok(houses) => houses,
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
}

#[server(GetActiveGuests)]
pub async fn get_active_guests() -> Result<Vec<Guest>, ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        get_all_active_guests(&mut conn).map_err(|e| ServerFnError::ServerError(e.to_string()))
    })
    .await;
    match result {
        Ok(guests) => guests,
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
}

#[server(GetUnregisteredGuests)]
pub async fn get_unregistered_guests() -> Result<Vec<Guest>, ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        get_all_unregistered_guests(&mut conn)
            .map_err(|e| ServerFnError::ServerError(e.to_string()))
    })
    .await;
    match result {
        Ok(guests) => guests,
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
}

#[server(GetCurrentUser)]
pub async fn get_current_user() -> Result<Option<Guest>, ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();

    use axum::http::HeaderMap;
    use leptos_axum::extract;

    let headers: HeaderMap = extract()
        .await
        .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;

    let mut token: Option<String> = None;
    if let Some(cookie_header) = headers.get(axum::http::header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix("session_token=") {
                    token = Some(value.to_string());
                    break;
                }
            }
        }
    }

    let result = tokio::task::spawn_blocking(
        move || -> Result<Option<Guest>, ServerFnError<NoCustomError>> {
            let mut conn = pool
                .get()
                .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
            if let Some(t) = token {
                Ok(get_guest_by_token(&mut conn, &t).ok())
            } else {
                Ok(None)
            }
        },
    )
    .await;

    match result {
        Ok(current_user) => current_user,
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
}

#[cfg(feature = "ssr")]
async fn extract_and_validate_admin_token(
    pool: DbPool,
) -> Result<Option<bool>, ServerFnError<NoCustomError>> {
    use axum::http::HeaderMap;
    use leptos_axum::extract;

    let headers: HeaderMap = extract()
        .await
        .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;

    let mut admin_token: Option<String> = None;
    if let Some(cookie_header) = headers.get(axum::http::header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix("admin_token=") {
                    admin_token = Some(value.to_string());
                    break;
                }
            }
        }
    }

    let result = tokio::task::spawn_blocking(
        move || -> Result<Option<bool>, ServerFnError<NoCustomError>> {
            let mut conn = pool
                .get()
                .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
            match admin_token {
                Some(t) => {
                    let is_valid = validate_admin_token(&mut conn, &t)
                        .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
                    Ok(Some(is_valid))
                }
                None => Ok(None),
            }
        },
    )
    .await;

    match result {
        Ok(validity) => validity,
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
}

// Checks if the current request is from an admin. Returns true if it is, false otherwise.
#[server(IsAdmin)]
pub async fn is_admin() -> Result<bool, ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();
    let validity = extract_and_validate_admin_token(pool).await?;
    Ok(validity.unwrap_or(false)) // None -> false
}

// Returns an empty result if the current request is from an admin, or an error otherwise.
#[cfg(feature = "ssr")]
async fn check_admin() -> Result<(), ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();
    let validity = extract_and_validate_admin_token(pool).await?;
    match validity {
        Some(true) => Ok(()),
        _ => Err(ServerFnError::ServerError("Unauthorized".to_string())),
    }
}

#[server(AdminLogin)]
pub async fn admin_login(password: String) -> Result<(), ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();
    let admin_password = env::var("ADMIN_PASSWORD").map_err(|_| {
        ServerFnError::<NoCustomError>::ServerError("Admin password not set".to_string())
    })?;

    if password != admin_password {
        return Err(ServerFnError::ServerError("Invalid password".to_string()));
    }

    let result =
        tokio::task::spawn_blocking(move || -> Result<String, ServerFnError<NoCustomError>> {
            let mut conn = pool
                .get()
                .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
            create_admin_session(&mut conn).map_err(|e| ServerFnError::ServerError(e.to_string()))
        })
        .await;

    let token =
        result.map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))??;

    use leptos_axum::ResponseOptions;
    let resp: ResponseOptions = expect_context();
    let cookie = format!(
        "admin_token={}; Max-Age=86400; Path=/; HttpOnly; SameSite=Strict",
        token
    );
    resp.insert_header(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&cookie)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?,
    );

    Ok(())
}

#[server(AdminLogout)]
pub async fn admin_logout() -> Result<(), ServerFnError<NoCustomError>> {
    use leptos_axum::ResponseOptions;
    let resp: ResponseOptions = expect_context();
    let cookie = "admin_token=; Max-Age=0; Path=/; HttpOnly; SameSite=Strict";
    resp.insert_header(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(cookie)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?,
    );
    Ok(())
}

#[server(RegisterGuest)]
pub async fn register_guest_handler(
    guest_id: i32,
    house_id: i32,
    character: String,
) -> Result<(String, i32), ServerFnError<NoCustomError>> {
    check_admin().await?;

    let pool: DbPool = expect_context();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        let effective_house_id = if house_id == 0 { None } else { Some(house_id) };
        let (guest, token) = register_guest(&mut conn, guest_id, effective_house_id, &character)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        let assigned_house_id = guest.house_id.unwrap();
        Ok((token, assigned_house_id))
    })
    .await;
    match result {
        Ok(token) => token,
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
}

#[server(UnregisterGuest)]
pub async fn unregister_guest_handler(guest_id: i32) -> Result<(), ServerFnError<NoCustomError>> {
    check_admin().await?;

    let pool: DbPool = expect_context();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        unregister_guest(&mut conn, guest_id)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        Ok(())
    })
    .await;
    result.map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?
}

#[server(ReregisterGuest)]
pub async fn reregister_guest_handler(
    guest_id: i32,
    new_house_id: Option<i32>,
    new_character: Option<String>,
) -> Result<String, ServerFnError<NoCustomError>> {
    check_admin().await?;

    let pool: DbPool = expect_context();

    let result =
        tokio::task::spawn_blocking(move || -> Result<String, ServerFnError<NoCustomError>> {
            let mut conn = pool
                .get()
                .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
            let (_, token) =
                reregister_guest(&mut conn, guest_id, new_house_id, new_character.as_deref())
                    .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
            Ok(token)
        })
        .await?;
    match result {
        Ok(token) => Ok(token),
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
}

#[server(AwardPointsToGuest)]
pub async fn award_points_to_guest_handler(
    guest_id: i32,
    amount: i32,
    reason: String,
) -> Result<(), ServerFnError<NoCustomError>> {
    check_admin().await?;

    let pool: DbPool = expect_context();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        award_points_to_guest(&mut conn, guest_id, amount, &reason)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        Ok(())
    })
    .await;
    result.map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?
}

#[server(AwardPointsToHouse)]
pub async fn award_points_to_house_handler(
    house_id: i32,
    amount: i32,
    reason: String,
) -> Result<(), ServerFnError<NoCustomError>> {
    check_admin().await?;

    let pool: DbPool = expect_context();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        award_points_to_house(&mut conn, house_id, amount, &reason)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        Ok(())
    })
    .await;
    result.map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?
}

#[server(GetGuestToken)]
pub async fn get_guest_token_handler(
    guest_id: i32,
) -> Result<String, ServerFnError<NoCustomError>> {
    check_admin().await?;

    let pool: DbPool = expect_context();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        get_guest_token(&mut conn, guest_id)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))
    })
    .await
    .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;

    match result {
        Ok(Some(token)) => Ok(token),
        Ok(None) => Err(ServerFnError::<NoCustomError>::ServerError(
            "No token found".to_string(),
        )),
        Err(e) => Err(e),
    }
}

#[server(GetPointAwards)]
pub async fn get_point_awards() -> Result<Vec<PointAwardLog>, ServerFnError<NoCustomError>> {
    check_admin().await?;

    let pool: DbPool = expect_context();

    tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        get_all_point_awards(&mut conn)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))
    })
    .await
    .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?
}

#[server(Login)]
pub async fn login_handler(
    guest_id: i32,
    token: String,
) -> Result<(), ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();

    use leptos_axum::ResponseOptions;

    let token_copy = token.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        let guest = get_guest_by_token(&mut conn, &token_copy)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        if guest.id != guest_id {
            return Err(ServerFnError::<NoCustomError>::ServerError(
                "Invalid guest or token".to_string(),
            ));
        }
        Ok(())
    })
    .await;
    result??;

    let resp: ResponseOptions = expect_context();
    let cookie = format!(
        "session_token={}; Max-Age=86400; Path=/; HttpOnly; SameSite=Strict",
        token
    );
    resp.insert_header(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&cookie)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?,
    );

    Ok(())
}

const WORDS: &[&str] = &[
    "apple", "bread", "break", "broad", "tread", "bleed", "dreab",
];

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/hp-halloween-25.css" />

        // sets the document title
        <Title text="Hogwarts Halloween Party" />

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=path!("/") view=Home />
                    <Route path=path!("/login") view=Login />
                    <Route path=path!("/admin/login") view=AdminLogin />
                    <Route path=path!("/admin") view=AdminDashboard />
                    <Route path=path!("/games/wordle") view=Wordle />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Home() -> impl IntoView {
    let houses = Resource::new(|| (), |_| get_houses());
    let current_user = Resource::new(|| (), |_| get_current_user());
    let is_admin_res = Resource::new(|| (), |_| is_admin());

    view! {
        <div>
            <h1>"Hogwarts Halloween Party"</h1>
            <Suspense fallback=|| {
                view! { "Loading..." }
            }>
                {move || {
                    houses
                        .with(|h_res| match h_res {
                            Some(Ok(houses)) => {
                                view! {
                                    <h2>"House Scores"</h2>
                                    <ul>
                                        {houses
                                            .iter()
                                            .map(|house| {
                                                view! { <li>{house.name.clone()}: {house.score}</li> }
                                            })
                                            .collect_view()}
                                    </ul>
                                }
                                    .into_any()
                            }
                            _ => view! { "Error loading houses" }.into_any(),
                        })
                }}
            </Suspense>
            <Suspense fallback=|| {
                view! { "Checking login..." }
            }>
                {move || {
                    current_user
                        .with(|u_res| match u_res {
                            Some(Ok(Some(guest))) => {
                                houses
                                    .with(|h_res| match h_res {
                                        Some(Ok(houses)) => {
                                            let house_opt = houses
                                                .iter()
                                                .find(|h| Some(h.id) == guest.house_id);
                                            let house_name = house_opt
                                                .map(|h| h.name.clone())
                                                .unwrap_or("Unknown".to_string());
                                            view! {
                                                <h2>
                                                    "Welcome, " {guest.name.clone()} " to " {house_name}
                                                </h2>
                                                <p>"Your personal score: " {guest.personal_score}</p>
                                                <h3>"Games and Activities"</h3>
                                                <ul>
                                                    <li>
                                                        <a href="/games/wordle">"Harry Potter Wordle"</a>
                                                    </li>
                                                // Add other games here as they are implemented
                                                </ul>
                                            }
                                                .into_any()
                                        }
                                        _ => view! { "Error loading houses" }.into_any(),
                                    })
                            }
                            _ => {
                                view! {
                                    <p>
                                        <a href="/login">"Login"</a>
                                    </p>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
            <Suspense>
                {move || {
                    is_admin_res
                        .with(|admin| match admin {
                            Some(Ok(true)) => {
                                view! {
                                    <p>
                                        <a href="/admin">"Admin Dashboard"</a>
                                    </p>
                                }
                                    .into_any()
                            }
                            _ => view! {}.into_any(),
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn Login() -> impl IntoView {
    let selected_guest = RwSignal::new(0i32);
    let token = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());

    let guests_resource = Resource::new(|| (), |_| get_active_guests());

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let g = selected_guest.get();
        let t = token.get();
        if g == 0 || t.is_empty() {
            error.set("Please select a guest and enter a token.".to_string());
            return;
        }
        spawn_local(async move {
            match login_handler(g, t).await {
                Ok(_) => {
                    error.set(String::new());
                    let navigate = use_navigate();
                    navigate("/", NavigateOptions::default());
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };

    view! {
        <div>
            <h1>"Login"</h1>
            <form on:submit=submit>
                <label>
                    "Guest: "
                    <select on:change=move |ev| {
                        let val = event_target_value(&ev).parse::<i32>().unwrap_or(0);
                        selected_guest.set(val);
                    }>
                        <option value="0">"Select your name"</option>
                        <Suspense fallback=|| {
                            view! { "Loading..." }
                        }>
                            {move || {
                                guests_resource
                                    .with(move |opt_res| {
                                        match opt_res {
                                            None => view! { "Loading..." }.into_any(),
                                            Some(res) => {
                                                match res {
                                                    Ok(guests) => {
                                                        guests
                                                            .iter()
                                                            .map(|guest| {
                                                                view! {
                                                                    <option value=guest
                                                                        .id
                                                                        .to_string()>{guest.name.clone()}</option>
                                                                }
                                                            })
                                                            .collect_view()
                                                            .into_any()
                                                    }
                                                    Err(e) => {
                                                        view! {
                                                            "Error loading guests: "
                                                            {e.to_string()}
                                                        }
                                                            .into_any()
                                                    }
                                                }
                                            }
                                        }
                                    })
                            }}
                        </Suspense>
                    </select>
                </label>
                <label>
                    "Token: "
                    <input type="text" on:input=move |ev| token.set(event_target_value(&ev)) />
                </label>
                <button type="submit">"Login"</button>
            </form>
            {move || (!error.get().is_empty()).then(|| view! { <p>{error.get()}</p> })}
        </div>
    }
}

#[component]
fn AdminLogin() -> impl IntoView {
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let p = password.get();
        if p.is_empty() {
            error.set("Please enter password.".to_string());
            return;
        }
        spawn_local(async move {
            match admin_login(p).await {
                Ok(_) => {
                    error.set(String::new());
                    let navigate = use_navigate();
                    navigate("/admin", NavigateOptions::default());
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };

    view! {
        <div>
            <h1>"Admin Login"</h1>
            <form on:submit=submit>
                <label>
                    "Password: "
                    <input
                        type="password"
                        on:input=move |ev| password.set(event_target_value(&ev))
                    />
                </label>
                <button type="submit">"Login"</button>
            </form>
            {move || {
                if !error.get().is_empty() {
                    view! { <p>{error.get()}</p> }.into_any()
                } else {
                    view! {}.into_any()
                }
            }}
        </div>
    }
}

#[component]
fn AdminDashboard() -> impl IntoView {
    // Fetchers for various resources (state).
    let is_admin_fetcher = Resource::new(|| (), |_| is_admin());
    let houses_fetcher = Resource::new(|| (), |_| get_houses());
    let active_guests_fetcher = Resource::new(|| (), |_| get_active_guests());
    let unregistered_guests_fetcher = Resource::new(|| (), |_| get_unregistered_guests());
    let point_awards_fetcher = Resource::new(|| (), |_| get_point_awards());

    // Runs on the next "tick" and redirects to the admin login page if the user is not an admin.
    // NOTE: This effect does not capture any reactive values, so it won't run again.
    // TODO: Instead, redirect to the homepage. It's better to not advertise the admin login page.
    let navigate = use_navigate();
    Effect::new(move || {
        is_admin_fetcher.with(|maybe_result| {
            if let Some(Ok(false)) = maybe_result {
                navigate("/admin/login", NavigateOptions::default());
            }
        });
    });

    // Signals related to registering a new guest.
    let selected_guest_id = RwSignal::new(0i32);
    let new_guest_character = RwSignal::new(String::new());
    let new_guest_house = RwSignal::new(0i32);
    let register_error = RwSignal::new(String::new());
    let registered_token = RwSignal::new(String::new());

    // A handler for the register new guest submit button. Attempts to register a new guest with
    // the provided details. On success, it clears any errors and sets the session token.
    let register_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let guest_id = selected_guest_id.get();
        let character = new_guest_character.get();
        let house_id = new_guest_house.get();
        if guest_id == 0 || character.is_empty() {
            register_error.set("Guest, character, and house are required.".to_string());
            return;
        }
        spawn_local(async move {
            match register_guest_handler(guest_id, house_id, character).await {
                Ok((token, assigned_house_id)) => {
                    register_error.set(String::new());
                    registered_token.set(token.clone());
                    selected_guest_id.set(0i32);
                    new_guest_character.set(String::new());

                    #[cfg(feature = "hydrate")]
                    {
                        // Trigger the sort server.
                        let sort_url =
                            format!("http://192.168.1.176/sort?house={}", assigned_house_id);
                        let window = web_sys::window().expect("window");

                        let mut init = web_sys::RequestInit::new();
                        init.set_method("GET");
                        init.set_mode(web_sys::RequestMode::NoCors);

                        let request =
                            web_sys::Request::new_with_str_and_init(&sort_url, &init).unwrap();

                        let resp_promise = window.fetch_with_request(&request);
                        let future = wasm_bindgen_futures::JsFuture::from(resp_promise);
                        log!(
                            "Sending request to Sorting Hat for house {}",
                            assigned_house_id
                        );
                        wasm_bindgen_futures::spawn_local(async move {
                            match future.await {
                                Ok(_) => log!(
                                    "Sort request sent successfully for house {}",
                                    assigned_house_id
                                ),
                                Err(e) => log!("Fetch error: {:?}", e),
                            }
                        });
                    }
                }
                Err(e) => register_error.set(e.to_string()),
            }
        });
    };

    // Signals related to awarding points to a guest.
    let award_guest_id = RwSignal::new(0i32);
    let award_guest_amount = RwSignal::new(0i32);
    let award_guest_reason = RwSignal::new(String::new());
    let award_guest_error = RwSignal::new(String::new());

    // A handler for the award guest points submit button. Attempts to award points to the specified
    // guest. On success, clears any errors and refreshes resources.
    let award_guest_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let guest_id = award_guest_id.get();
        let amount = award_guest_amount.get();
        let reason = award_guest_reason.get();
        if guest_id == 0 || reason.is_empty() {
            award_guest_error.set("Guest and reason are required.".to_string());
            return;
        }
        if amount == 0 {
            award_guest_error.set("Amount cannot be zero.".to_string());
            return;
        }
        spawn_local(async move {
            match award_points_to_guest_handler(guest_id, amount, reason).await {
                Ok(_) => {
                    award_guest_error.set(String::new());
                    award_guest_id.set(0i32);
                    award_guest_amount.set(0i32);
                    award_guest_reason.set(String::new());

                    active_guests_fetcher.refetch();
                    houses_fetcher.refetch();
                    point_awards_fetcher.refetch();
                }
                Err(e) => award_guest_error.set(e.to_string()),
            }
        });
    };

    // Signals related to awarding points to a house.
    let award_house_id = RwSignal::new(0i32);
    let award_house_amount = RwSignal::new(0i32);
    let award_house_reason = RwSignal::new(String::new());
    let award_house_error = RwSignal::new(String::new());

    // A handler for the award house points submit button. Attempts to award points to the specified
    // house. On success, clears any errors and refreshes resources.
    let award_house_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let house_id = award_house_id.get();
        let amount = award_house_amount.get();
        let reason = award_house_reason.get();
        if house_id == 0 || reason.is_empty() {
            award_house_error.set("House and reason are required.".to_string());
            return;
        }
        if amount == 0 {
            award_house_error.set("Amount cannot be zero.".to_string());
            return;
        }
        spawn_local(async move {
            match award_points_to_house_handler(house_id, amount, reason).await {
                Ok(_) => {
                    award_house_error.set(String::new());
                    award_house_id.set(0i32);
                    award_house_amount.set(0i32);
                    award_house_reason.set(String::new());

                    active_guests_fetcher.refetch();
                    houses_fetcher.refetch();
                    point_awards_fetcher.refetch();
                }
                Err(e) => award_house_error.set(e.to_string()),
            }
        });
    };

    let unregister = move |guest_id: i32| {
        spawn_local(async move {
            log!("Unregistering");
            if leptos::leptos_dom::helpers::window()
                .confirm_with_message("Unregister this guest?")
                .unwrap_or(false)
            {
                match unregister_guest_handler(guest_id).await {
                    Ok(_) => active_guests_fetcher.refetch(),
                    Err(e) => log!("Error: {}", e),
                }
            }
        });
    };

    let show_token = move |guest_id: i32| {
        spawn_local(async move {
            match get_guest_token_handler(guest_id).await {
                Ok(token) => {
                    leptos::leptos_dom::helpers::window()
                        .alert_with_message(&format!("Token: {}", token))
                        .unwrap_or_default();
                }
                Err(e) => {
                    log!("Error fetching token: {}", e);
                    leptos::leptos_dom::helpers::window()
                        .alert_with_message(&format!("Error: {}", e))
                        .unwrap_or_default();
                }
            }
        });
    };

    let logout = move |_| {
        spawn_local(async move {
            let _ = admin_logout().await;
            let navigate = use_navigate();
            navigate("/", NavigateOptions::default());
        });
    };

    view! {
        <Suspense fallback=|| {
            "Loading..."
        }>
            {move || {
                if let Some(Ok(true)) = is_admin_fetcher.get() {
                    view! {
                        <div class="admin-container">
                            <header class="admin-header">
                                <h1>"Admin Dashboard"</h1>
                                <button class="btn-logout" on:click=logout>
                                    "Logout"
                                </button>
                            </header>

                            <section class="admin-section">
                                <h2>"Register New Guest"</h2>
                                <form class="admin-form" on:submit=register_submit>
                                    <div class="form-group">
                                        <label>
                                            "Guest: "
                                            <select
                                                class="form-select"
                                                prop:value=move || selected_guest_id.get().to_string()
                                                on:change=move |ev| {
                                                    selected_guest_id
                                                        .set(event_target_value(&ev).parse().unwrap_or(0))
                                                }
                                            >
                                                <option value="0">"Select guest"</option>
                                                <Suspense fallback=|| {
                                                    view! { <option>"Loading..."</option> }
                                                }>
                                                    {move || {
                                                        unregistered_guests_fetcher
                                                            .with(|maybe_result| match maybe_result {
                                                                Some(Ok(guests)) => {
                                                                    let mut sorted_guests = guests.clone();
                                                                    sorted_guests
                                                                        .sort_by(|a, b| {
                                                                            a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                                                        });
                                                                    sorted_guests
                                                                        .iter()
                                                                        .map(|guest| {
                                                                            view! {
                                                                                <option value=guest
                                                                                    .id
                                                                                    .to_string()>{guest.name.clone()}</option>
                                                                            }
                                                                        })
                                                                        .collect_view()
                                                                        .into_any()
                                                                }
                                                                _ => view! { <option>"Error"</option> }.into_any(),
                                                            })
                                                    }}
                                                </Suspense>
                                            </select>
                                        </label>
                                    </div>
                                    <div class="form-group">
                                        <label>
                                            "Character: "
                                            <input
                                                class="form-input"
                                                type="text"
                                                placeholder="e.g., Harry Potter"
                                                prop:value=move || new_guest_character.get()
                                                on:input=move |ev| {
                                                    new_guest_character.set(event_target_value(&ev))
                                                }
                                            />
                                        </label>
                                    </div>
                                    <div class="form-group">
                                        <label>
                                            "House: "
                                            <select
                                                class="form-select"
                                                prop:value=move || new_guest_house.get().to_string()
                                                on:change=move |ev| {
                                                    new_guest_house
                                                        .set(event_target_value(&ev).parse().unwrap_or(1))
                                                }
                                            >
                                                <option value="0">"Sorting Hat"</option>
                                                <Suspense fallback=|| {
                                                    view! { <option>"Loading..."</option> }
                                                }>
                                                    {move || {
                                                        houses_fetcher
                                                            .with(|maybe_result| match maybe_result {
                                                                Some(Ok(houses)) => {
                                                                    houses
                                                                        .iter()
                                                                        .map(|house| {
                                                                            view! {
                                                                                <option value=house
                                                                                    .id
                                                                                    .to_string()>{house.name.clone()}</option>
                                                                            }
                                                                        })
                                                                        .collect_view()
                                                                        .into_any()
                                                                }
                                                                _ => view! { <option>"Error"</option> }.into_any(),
                                                            })
                                                    }}
                                                </Suspense>
                                            </select>
                                        </label>
                                    </div>
                                    <button type="submit" class="btn-primary">
                                        "Sort"
                                    </button>
                                </form>
                                {move || {
                                    if !register_error.get().is_empty() {
                                        view! { <p class="error">{register_error.get()}</p> }
                                            .into_any()
                                    } else {
                                        view! {}.into_any()
                                    }
                                }}
                                {move || {
                                    if !registered_token.get().is_empty() {
                                        view! {
                                            <p class="token-display">
                                                "Token: " {registered_token.get()}
                                            </p>
                                        }
                                            .into_any()
                                    } else {
                                        view! {}.into_any()
                                    }
                                }}
                            </section>

                            <section class="admin-section">
                                <h2>"Award Points to Guest"</h2>
                                <form class="admin-form" on:submit=award_guest_submit>
                                    <div class="form-group">
                                        <label>
                                            "Guest: "
                                            <select
                                                class="form-select"
                                                prop:value=move || award_guest_id.get().to_string()
                                                on:change=move |ev| {
                                                    award_guest_id
                                                        .set(event_target_value(&ev).parse().unwrap_or(0))
                                                }
                                            >
                                                <option value="0">"Select guest"</option>
                                                <Suspense fallback=|| {
                                                    view! { <option>"Loading..."</option> }
                                                }>
                                                    {move || {
                                                        active_guests_fetcher
                                                            .with(|maybe_result| match maybe_result {
                                                                Some(Ok(guests)) => {
                                                                    guests
                                                                        .iter()
                                                                        .map(|guest| {
                                                                            view! {
                                                                                <option value=guest
                                                                                    .id
                                                                                    .to_string()>{guest.name.clone()}</option>
                                                                            }
                                                                        })
                                                                        .collect_view()
                                                                        .into_any()
                                                                }
                                                                _ => view! { <option>"Error"</option> }.into_any(),
                                                            })
                                                    }}
                                                </Suspense>
                                            </select>
                                        </label>
                                    </div>
                                    <div class="form-group">
                                        <label>
                                            "Amount: "
                                            <input
                                                class="form-input"
                                                type="number"
                                                prop:value=move || format!("{}", award_guest_amount.get())
                                                on:input=move |ev| {
                                                    if let Ok(value) = event_target_value(&ev).parse::<i32>() {
                                                        award_guest_amount.set(value);
                                                    }
                                                }
                                            />
                                        </label>
                                    </div>
                                    <div class="form-group">
                                        <label>
                                            "Reason: "
                                            <input
                                                class="form-input"
                                                type="text"
                                                prop:value=move || award_guest_reason.get()
                                                on:input=move |ev| {
                                                    award_guest_reason.set(event_target_value(&ev))
                                                }
                                            />
                                        </label>
                                    </div>
                                    <button type="submit" class="btn-primary">
                                        "Award Points"
                                    </button>
                                </form>
                                {move || {
                                    if !award_guest_error.get().is_empty() {
                                        view! { <p class="error">{award_guest_error.get()}</p> }
                                            .into_any()
                                    } else {
                                        view! {}.into_view().into_any()
                                    }
                                }}
                            </section>

                            <section class="admin-section">
                                <h2>"Award Points to House"</h2>
                                <form class="admin-form" on:submit=award_house_submit>
                                    <div class="form-group">
                                        <label>
                                            "House: "
                                            <select
                                                class="form-select"
                                                prop:value=move || award_house_id.get().to_string()
                                                on:change=move |ev| {
                                                    award_house_id
                                                        .set(event_target_value(&ev).parse().unwrap_or(0))
                                                }
                                            >
                                                <option value="0">"Select house"</option>
                                                <Suspense fallback=|| {
                                                    view! { <option>"Loading..."</option> }
                                                }>
                                                    {move || {
                                                        houses_fetcher
                                                            .with(|maybe_result| match maybe_result {
                                                                Some(Ok(houses)) => {
                                                                    houses
                                                                        .iter()
                                                                        .map(|house| {
                                                                            view! {
                                                                                <option value=house
                                                                                    .id
                                                                                    .to_string()>{house.name.clone()}</option>
                                                                            }
                                                                        })
                                                                        .collect_view()
                                                                        .into_any()
                                                                }
                                                                _ => view! { <option>"Error"</option> }.into_any(),
                                                            })
                                                    }}
                                                </Suspense>
                                            </select>
                                        </label>
                                    </div>
                                    <div class="form-group">
                                        <label>
                                            "Amount: "
                                            <input
                                                class="form-input"
                                                type="number"
                                                prop:value=move || format!("{}", award_house_amount.get())
                                                on:input=move |ev| {
                                                    if let Ok(value) = event_target_value(&ev).parse::<i32>() {
                                                        award_house_amount.set(value);
                                                    }
                                                }
                                            />
                                        </label>
                                    </div>
                                    <div class="form-group">
                                        <label>
                                            "Reason: "
                                            <input
                                                class="form-input"
                                                prop:value=move || award_house_reason.get()
                                                type="text"
                                                on:input=move |ev| {
                                                    award_house_reason.set(event_target_value(&ev))
                                                }
                                            />
                                        </label>
                                    </div>
                                    <button type="submit" class="btn-primary">
                                        "Award Points"
                                    </button>
                                </form>
                                {move || {
                                    if !award_house_error.get().is_empty() {
                                        view! { <p class="error">{award_house_error.get()}</p> }
                                            .into_any()
                                    } else {
                                        view! {}.into_view().into_any()
                                    }
                                }}
                            </section>

                            <section class="admin-section">
                                <h2>"Active Guests"</h2>
                                <div class="table-responsive">
                                    <table class="admin-table">
                                        <tbody>
                                            <tr>
                                                <th>"ID"</th>
                                                <th>"Name"</th>
                                                <th>"House"</th>
                                                <th>"Score"</th>
                                                <th>"Actions"</th>
                                            </tr>
                                            <Suspense fallback=|| {
                                                view! {
                                                    <tr>
                                                        <td colspan="5">"Loading..."</td>
                                                    </tr>
                                                }
                                            }>
                                                {move || {
                                                    active_guests_fetcher
                                                        .with(|maybe_result| match maybe_result {
                                                            Some(Ok(guests)) => {
                                                                if guests.is_empty() {
                                                                    return view! {
                                                                        <tr>
                                                                            <td colspan="5">"No active guests"</td>
                                                                        </tr>
                                                                    }
                                                                        .into_any();
                                                                }
                                                                guests
                                                                    .iter()
                                                                    .map(|guest| {
                                                                        let id = guest.id;
                                                                        view! {
                                                                            <tr>
                                                                                <td>{format!("{}", guest.id)}</td>
                                                                                <td>{guest.name.clone()}</td>
                                                                                <td>
                                                                                    {houses_fetcher
                                                                                        .with(|maybe_result| {
                                                                                            maybe_result
                                                                                                .as_ref()
                                                                                                .and_then(|result| result.as_ref().ok())
                                                                                                .and_then(|houses| {
                                                                                                    houses.iter().find(|house| Some(house.id) == guest.house_id)
                                                                                                })
                                                                                                .map(|house| house.name.clone())
                                                                                                .unwrap_or_else(|| "Unknown".to_string())
                                                                                        })}
                                                                                </td>
                                                                                <td>{format!("{}", guest.personal_score)}</td>
                                                                                <td>
                                                                                    <button
                                                                                        class="btn-secondary"
                                                                                        on:click=move |_| show_token(id)
                                                                                    >
                                                                                        "Show token"
                                                                                    </button>
                                                                                    <button class="btn-danger" on:click=move |_| unregister(id)>
                                                                                        "Unregister"
                                                                                    </button>
                                                                                </td>
                                                                            </tr>
                                                                        }
                                                                    })
                                                                    .collect_view()
                                                                    .into_any()
                                                            }
                                                            _ => {
                                                                view! {
                                                                    <tr>
                                                                        <td colspan="5">"Loading..."</td>
                                                                    </tr>
                                                                }
                                                                    .into_view()
                                                                    .into_any()
                                                            }
                                                        })
                                                }}
                                            </Suspense>
                                        </tbody>
                                    </table>
                                </div>
                            </section>

                            <section class="admin-section">
                                <h2>"Point Awards History"</h2>
                                <div class="table-responsive">
                                    <table class="admin-table">
                                        <tbody>
                                            <tr>
                                                <th>ID</th>
                                                <th>Guest</th>
                                                <th>House</th>
                                                <th>Amount</th>
                                                <th>Reason</th>
                                                <th>Time</th>
                                            </tr>
                                            <Suspense>
                                                {move || {
                                                    point_awards_fetcher
                                                        .with(|maybe_result| match maybe_result {
                                                            Some(Ok(awards)) => {
                                                                awards
                                                                    .iter()
                                                                    .map(|award| {
                                                                        view! {
                                                                            <tr>
                                                                                <td>{award.id}</td>
                                                                                <td>
                                                                                    {award.guest_name.clone().unwrap_or("N/A".to_string())}
                                                                                </td>
                                                                                <td>
                                                                                    {award.house_name.clone().unwrap_or("N/A".to_string())}
                                                                                </td>
                                                                                <td>{award.amount}</td>
                                                                                <td>{award.reason.clone()}</td>
                                                                                <td>{award.awarded_at.to_string()}</td>
                                                                            </tr>
                                                                        }
                                                                    })
                                                                    .collect_view()
                                                                    .into_any()
                                                            }
                                                            _ => view! {}.into_view().into_any(),
                                                        })
                                                }}
                                            </Suspense>
                                        </tbody>
                                    </table>
                                </div>
                            </section>
                        </div>
                    }
                        .into_any()
                } else {
                    view! { "Loading..." }.into_any()
                }
            }}
        </Suspense>
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum LetterStatus {
    Correct, // green: right letter, right position
    Present, // yellow: right letter, wrong position
    Absent,  // gray: wrong letter
    Unused,  // default for keyboard
}

/// Renders the home page of your application.
#[component]
fn Wordle() -> impl IntoView {
    let target_word = RwSignal::new(String::new());
    let guesses = RwSignal::new(vec![] as Vec<String>);
    let current_guess = RwSignal::new(String::new());
    let keyboard_status = RwSignal::new(HashMap::<char, LetterStatus>::new());
    let game_over = RwSignal::new(false);
    let message = RwSignal::new(String::new());

    Effect::new(move || {
        let mut rng = rng();
        let word = WORDS.choose(&mut rng).unwrap_or(&"apple").to_uppercase();
        target_word.set(word);
        log!("Target word: {}", target_word.get());
    });

    let grid = move || {
        let mut rows = vec![];
        for i in 0..6 {
            let row_guess = if i < guesses.get().len() {
                guesses.get()[i].clone()
            } else if i == guesses.get().len() {
                current_guess.get()
            } else {
                String::from("     ")
            };

            let statuses = if i < guesses.get().len() {
                compute_statuses(&row_guess, &target_word.get())
            } else {
                vec![LetterStatus::Unused; 5]
            };

            rows.push(view! {
                <div class="row">
                    {(0..5)
                        .map(|j| {
                            let letter = row_guess.chars().nth(j).unwrap_or(' ');
                            let status = statuses.get(j).cloned().unwrap_or(LetterStatus::Unused);
                            let class = match status {
                                LetterStatus::Correct => "correct",
                                LetterStatus::Present => "present",
                                LetterStatus::Absent => "absent",
                                _ => "",
                            };
                            view! { <div class=class>{letter}</div> }
                        })
                        .collect::<Vec<_>>()}
                </div>
            });
        }
        rows
    };

    let keyboard = move || {
        let rows = ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"];
        view! {
            <div class="keyboard">
                {rows
                    .iter()
                    .map(|&row_str| {
                        view! {
                            <div class="keyboard-row">
                                {move || {
                                    if row_str == "ZXCVBNM" {
                                        view! {
                                            <button
                                                class="special"
                                                on:click=move |_| {
                                                    if game_over.get() || guesses.get().len() >= 6 {
                                                        return;
                                                    }
                                                    let guess = current_guess.get();
                                                    if guess.len() == 5
                                                        && WORDS.contains(&guess.to_lowercase().as_str())
                                                    {
                                                        process_guess(
                                                            guess.clone(),
                                                            target_word.get(),
                                                            guesses,
                                                            current_guess,
                                                            keyboard_status,
                                                            game_over,
                                                            message,
                                                        );
                                                    } else {
                                                        log!("Invalid word");
                                                    }
                                                }
                                            >
                                                "Enter"
                                            </button>
                                        }
                                            .into_any()
                                    } else {
                                        view! { <div style="width: 60px;" /> }.into_any()
                                    }
                                }}
                                {row_str
                                    .chars()
                                    .map(|k| {
                                        let status = move || {
                                            keyboard_status
                                                .get()
                                                .get(&k)
                                                .cloned()
                                                .unwrap_or(LetterStatus::Unused)
                                        };
                                        let class = move || match status() {
                                            LetterStatus::Correct => "correct",
                                            LetterStatus::Present => "present",
                                            LetterStatus::Absent => "absent",
                                            LetterStatus::Unused => "",
                                        };
                                        view! {
                                            <button
                                                class=class
                                                on:click=move |_| {
                                                    if game_over.get() || guesses.get().len() >= 6 {
                                                        return;
                                                    }
                                                    if current_guess.get().len() < 5 {
                                                        current_guess.update(|g| g.push(k));
                                                    }
                                                }
                                            >
                                                {k}
                                            </button>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                                {move || {
                                    if row_str == "ZXCVBNM" {
                                        view! {
                                            <button
                                                class="special"
                                                on:click=move |_| {
                                                    if game_over.get() || guesses.get().len() >= 6 {
                                                        return;
                                                    }
                                                    current_guess
                                                        .update(|g| {
                                                            if !g.is_empty() {
                                                                g.pop();
                                                            }
                                                        });
                                                }
                                            >
                                                "⌫"
                                            </button>
                                        }
                                            .into_any()
                                    } else {
                                        view! { <div style="width: 60px;" /> }.into_any()
                                    }
                                }}
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        }
    };

    view! {
        <div class="wordle">
            <h1>"Wordle"</h1>
            <div class="grid">{grid}</div>
            <p>{move || message.get()}</p>
            {keyboard}
        </div>
    }
}

fn compute_statuses(guess: &str, target: &str) -> Vec<LetterStatus> {
    let mut statuses = vec![LetterStatus::Absent; 5];
    let mut target_counts: HashMap<char, usize> = HashMap::new();
    for c in target.chars() {
        *target_counts.entry(c).or_insert(0) += 1;
    }

    // First pass: Correct positions.
    for (i, c) in guess.chars().enumerate() {
        if target.chars().nth(i) == Some(c) {
            statuses[i] = LetterStatus::Correct;
            *target_counts.entry(c).or_insert(0) -= 1;
        }
    }

    // Second pass: Present but wrong position.
    for (i, c) in guess.chars().enumerate() {
        if statuses[i] != LetterStatus::Correct && target_counts.get(&c).unwrap_or(&0) > &0 {
            statuses[i] = LetterStatus::Present;
            *target_counts.entry(c).or_insert(0) -= 1;
        }
    }

    statuses
}

fn process_guess(
    guess: String,
    target: String,
    guesses: RwSignal<Vec<String>>,
    current_guess: RwSignal<String>,
    keyboard_status: RwSignal<HashMap<char, LetterStatus>>,
    game_over: RwSignal<bool>,
    message: RwSignal<String>,
) {
    guesses.update(|gs| gs.push(guess.clone()));
    current_guess.set(String::new());

    // Update keyboard statuses.
    let statuses = compute_statuses(&guess, &target);
    keyboard_status.update(|ks| {
        for (i, c) in guess.chars().enumerate() {
            let new_status = statuses[i];
            let current = ks.get(&c).cloned().unwrap_or(LetterStatus::Unused);
            // Priority: Current > Present > Absent.
            let updated = match (current, new_status) {
                (_, LetterStatus::Correct) => LetterStatus::Correct,
                (LetterStatus::Unused, LetterStatus::Present) => LetterStatus::Present,
                (LetterStatus::Absent, LetterStatus::Present) => LetterStatus::Present,
                (_, LetterStatus::Absent)
                    if current != LetterStatus::Correct && current != LetterStatus::Present =>
                {
                    LetterStatus::Absent
                }
                _ => current,
            };
            ks.insert(c, updated);
        }
    });

    // Check win/loss.
    if guess == target {
        game_over.set(true);
        message.set("You win!".to_string());
    } else if guesses.get().len() >= 6 {
        game_over.set(true);
        message.set(format!("Game over! The word was {}", target));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_statuses() {
        // Exact match.
        assert_eq!(
            compute_statuses("APPLE", "APPLE"),
            vec![LetterStatus::Correct; 5]
        );

        // All absent.
        assert_eq!(
            compute_statuses("APPLE", "CROWD"),
            vec![LetterStatus::Absent; 5]
        );

        // All present but jumbled with some correct.
        assert_eq!(
            compute_statuses("STARE", "TEARS"),
            vec![
                LetterStatus::Present,
                LetterStatus::Present,
                LetterStatus::Correct,
                LetterStatus::Correct,
                LetterStatus::Present,
            ]
        );

        // Partial match with duplicates.
        assert_eq!(
            compute_statuses("PAPER", "APPLE"),
            vec![
                LetterStatus::Present,
                LetterStatus::Present,
                LetterStatus::Correct,
                LetterStatus::Present,
                LetterStatus::Absent
            ]
        );

        // Duplicates exceeding target count.
        assert_eq!(
            compute_statuses("AAABB", "AACDD"),
            vec![
                LetterStatus::Correct,
                LetterStatus::Correct,
                LetterStatus::Absent,
                LetterStatus::Absent,
                LetterStatus::Absent
            ]
        );
    }

    #[test]
    #[ignore]
    fn test_process_guess() {
        let target = "APPLE".to_string();
        let guesses = RwSignal::new(vec![]);
        let current_guess = RwSignal::new("BREAD".to_string());
        let keyboard_status = RwSignal::new(HashMap::new());
        let game_over = RwSignal::new(false);
        let message = RwSignal::new(String::new());

        process_guess(
            "BREAD".to_string(),
            target.clone(),
            guesses,
            current_guess,
            keyboard_status,
            game_over,
            message,
        );

        assert_eq!(guesses.get(), vec!["BREAD".to_string()]);
        assert_eq!(current_guess.get(), String::new());
        assert!(!game_over.get());
        assert_eq!(message.get(), String::new());

        // Check keyboard updates.
        let ks = keyboard_status.get();
        assert_eq!(ks.get(&'A'), Some(&LetterStatus::Present));
        assert_eq!(ks.get(&'B'), Some(&LetterStatus::Absent));
        assert_eq!(ks.get(&'D'), Some(&LetterStatus::Absent));
        assert_eq!(ks.get(&'E'), Some(&LetterStatus::Present));
        assert_eq!(ks.get(&'R'), Some(&LetterStatus::Absent));

        // Win case.
        let current_guess = RwSignal::new("APPLE".to_string());
        process_guess(
            "APPLE".to_string(),
            target.clone(),
            guesses,
            current_guess,
            keyboard_status,
            game_over,
            message,
        );
        assert!(game_over.get());
        assert_eq!(message.get(), "You win!");

        // Check keyboard updates.
        let ks = keyboard_status.get();
        assert_eq!(ks.get(&'A'), Some(&LetterStatus::Correct));
        assert_eq!(ks.get(&'B'), Some(&LetterStatus::Absent));
        assert_eq!(ks.get(&'D'), Some(&LetterStatus::Absent));
        assert_eq!(ks.get(&'E'), Some(&LetterStatus::Correct));
        assert_eq!(ks.get(&'L'), Some(&LetterStatus::Correct));
        assert_eq!(ks.get(&'P'), Some(&LetterStatus::Correct));
        assert_eq!(ks.get(&'R'), Some(&LetterStatus::Absent));

        // Loss case (simulate 6 guesses).
        let guesses = RwSignal::new(vec!["WRONG".to_string(); 5]);
        let current_guess = RwSignal::new("WRONG".to_string());
        process_guess(
            "WRONG".to_string(),
            target,
            guesses,
            current_guess,
            keyboard_status,
            game_over,
            message,
        );
        assert_eq!(guesses.get().len(), 6);
        assert!(game_over.get());
        assert!(message.get().contains("Game over!"));
    }
}
