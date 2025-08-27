use leptos::logging::log;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};
use rand::prelude::*;
use rand::rng;
use std::collections::HashMap;

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
        <Title text="Wordle" />

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=Wordle />
                </Routes>
            </main>
        </Router>
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

    Effect::new(move |_| {
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
