use leptos::ev::SubmitEvent;
use leptos::logging::log;
use leptos::prelude::*;
use leptos::server_fn::error::NoCustomError;
use leptos::task::spawn_local;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_navigate,
    path, NavigateOptions, StaticSegment,
};
use rand::prelude::*;
use rand::rng;
use std::collections::HashMap;

use crate::model::{Guest, House};
#[cfg(feature = "ssr")]
use crate::{get_all_active_guests, get_all_houses, get_guest_by_token, register_guest};

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

#[server(RegisterGuest)]
pub async fn register_guest_handler(
    name: String,
    house_id: i32,
) -> Result<String, ServerFnError<NoCustomError>> {
    let pool: DbPool = expect_context();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .get()
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        let (_, token) = register_guest(&mut conn, &name, house_id)
            .map_err(|e| ServerFnError::<NoCustomError>::ServerError(e.to_string()))?;
        Ok(token)
    })
    .await;
    match result {
        Ok(token) => token,
        Err(e) => Err(ServerFnError::ServerError(e.to_string())),
    }
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
                    <Route path=StaticSegment("/") view=Home />
                    <Route path=StaticSegment("/login") view=Login />
                    <Route path=StaticSegment("/register") view=RegisterGuestComponent />
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
                                                .find(|h| h.id == guest.house_id);
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
                        <option value="0">"Select yout name"</option>
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
fn RegisterGuestComponent() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let selected_house = RwSignal::new(0i32);
    let token = RwSignal::new(Option::<String>::None);
    let error = RwSignal::new(String::new());

    let houses_resource = Resource::new(|| (), move |_| get_houses());

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let n = name.get();
        let h = selected_house.get();
        if n.is_empty() || h == 0 {
            error.set("Please enter a name and select a house.".to_string());
            return;
        }
        spawn_local(async move {
            match register_guest_handler(n, h).await {
                Ok(t) => {
                    token.set(Some(t));
                    error.set(String::new());
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };

    view! {
        <div>
            <h1>"Register Guest"</h1>
            <form on:submit=submit>
                <label>
                    "Name: "
                    <input type="text" on:input=move |ev| name.set(event_target_value(&ev)) />
                </label>
                <label>
                    "House:"
                    <select on:change=move |ev| {
                        let val = event_target_value(&ev).parse::<i32>().unwrap_or(0);
                        selected_house.set(val);
                    }>
                        <option value="0">"Select a house"</option>
                        <Suspense fallback=|| {
                            view! { "Loading..." }
                        }>
                            {move || {
                                houses_resource
                                    .with(move |opt_res| {
                                        match opt_res {
                                            None => view! { "Loading..." }.into_any(),
                                            Some(res) => {
                                                match res {
                                                    Ok(houses) => {
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
                                                    Err(e) => {
                                                        view! {
                                                            "Error loading houses: "
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
                <button type="submit">"Submit"</button>
            </form>
            {move || error.get().is_empty().then_some(view! { <p>{error.get()}</p> })}
            {move || token.get().map(|t| view! { <p>"Token: " {t}</p> })}
        </div>
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
