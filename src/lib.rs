pub mod app;
pub mod model;
#[cfg(feature = "ssr")]
pub mod schema;

#[cfg(feature = "ssr")]
use chrono::Utc;
#[cfg(feature = "ssr")]
use diesel::connection::SimpleConnection;
#[cfg(feature = "ssr")]
use diesel::prelude::*;
#[cfg(feature = "ssr")]
use diesel::SqliteConnection;
#[cfg(feature = "ssr")]
use dotenvy::dotenv;
#[cfg(feature = "ssr")]
use rand::distr::weighted::WeightedIndex;
use rand::prelude::*;
#[cfg(feature = "ssr")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "ssr")]
use std::env;
#[cfg(feature = "ssr")]
use std::io::{Error as IoError, ErrorKind};
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
use crate::model::{
    CrosswordState, DbCrosswordState, Guest, House, HouseCrosswordCompletion, NewAdminSession,
    NewDbCrosswordState, NewHouseCrosswordCompletion, NewPointAward, NewSession, NewVote,
    NewVotingStatus, PointAward, PointAwardLog, RcvResult, RcvRound, Vote, VotingStatus,
};
#[cfg(feature = "ssr")]
use crate::schema::{
    admin_sessions, crossword_states, guests, house_crossword_completions, houses, point_awards,
    sessions, votes, voting_status,
};

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

#[cfg(feature = "ssr")]
pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");
    let mut conn = SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url));

    // Enable WAL mode to allow concurrent reads during writes, and a timeout to retry locked
    // operations.
    conn.batch_execute(
        "PRAGMA foreign_keys = ON; \
        PRAGMA journal_mode = WAL; \
        PRAGMA synchronous = NORMAL; \
        PRAGMA busy_timeout = 10000;",
    )
    .expect("Failed to set SQLite PRAGMAs");

    conn
}

/// Registers a guest by ID (prepopulated unregistered guest), assigns them to a house, sets their
/// character, sets registered_at to now, activates them, and generates a session token.
/// Errors if guest doesn't exist or is already active.
/// Returns the updated guest and token string.
#[cfg(feature = "ssr")]
pub fn register_guest(
    conn: &mut SqliteConnection,
    guest_id: i32,
    house_id: Option<i32>,
    character: &str,
) -> Result<(Guest, String), diesel::result::Error> {
    conn.transaction(|conn| {
        // Fetch the existing guest and ensure it's inactive.
        let existing_guest: Guest = guests::table
            .filter(guests::id.eq(guest_id))
            .select(Guest::as_select())
            .first(conn)?;
        if existing_guest.is_active == 1 {
            return Err(diesel::result::Error::QueryBuilderError(Box::new(
                IoError::new(ErrorKind::Other, "Guest already active"),
            )));
        }

        let final_house_id = if let Some(provided_house_id) = house_id {
            let house_exists: i64 = houses::table
                .filter(houses::id.eq(provided_house_id))
                .count()
                .get_result(conn)?;
            if house_exists == 0 {
                return Err(diesel::result::Error::NotFound);
            }
            provided_house_id
        } else {
            // Assert that we're working with 37 guests, for simplicity.
            let total_guests: i64 = guests::table.count().get_result(conn)?;
            if total_guests != 37 {
                return Err(diesel::result::Error::QueryBuilderError(Box::new(
                    IoError::new(
                        ErrorKind::Other,
                        "Expected exactly 37 guests in the database",
                    ),
                )));
            }

            // Based on how many have been sorted, determine how many we're targeting in each
            // house.
            let sorted_so_far: i64 = guests::table
                .filter(guests::is_active.eq(1i32))
                .count()
                .get_result(conn)?;
            let targets: Vec<i64> = if sorted_so_far < 18 {
                vec![4, 5, 5, 4]
            } else {
                vec![10, 9, 9, 9]
            };

            // Load the house ids in order.
            let house_ids: Vec<i32> = houses::table
                .select(houses::id)
                .order(houses::id.asc())
                .load(conn)?;
            if house_ids.len() != 4 {
                return Err(diesel::result::Error::QueryBuilderError(Box::new(
                    IoError::new(ErrorKind::Other, "Expected exactly 4 houses"),
                )));
            }

            // Compute current counts for each house, and subsequently, the remaining spots in each
            // house.
            let mut current_counts: Vec<i64> = Vec::new();
            for &house_id in &house_ids {
                let count: i64 = guests::table
                    .filter(guests::is_active.eq(1i32))
                    .filter(guests::house_id.eq(Some(house_id)))
                    .count()
                    .get_result(conn)?;
                current_counts.push(count);
            }
            let remainings: Vec<i64> = targets
                .iter()
                .zip(current_counts.iter())
                .map(|(&target, &current)| (target - current).max(0))
                .collect();

            // Create a distribution of the houses weighted by the number of spots left in each
            // house.
            let dist = WeightedIndex::new(
                remainings
                    .iter()
                    .map(|&w| w as usize)
                    .collect::<Vec<usize>>(),
            )
            .map_err(|e| {
                diesel::result::Error::QueryBuilderError(Box::new(IoError::new(
                    ErrorKind::Other,
                    format!("WeightedIndex error: {}", e),
                )))
            })?;

            // Sample the house id randomly from that distribution.
            let mut rng = rand::rng();
            house_ids[dist.sample(&mut rng)]
        };

        // Update the guest: set house, character, registered_at, and activate.
        let now = Utc::now().naive_utc();
        diesel::update(guests::table.filter(guests::id.eq(guest_id)))
            .set((
                guests::house_id.eq(Some(final_house_id)),
                guests::character.eq(Some(character.to_string())),
                guests::registered_at.eq(Some(now)),
                guests::is_active.eq(1i32),
            ))
            .execute(conn)?;

        // Delete any old sessions (though unlikely for inactive).
        diesel::delete(sessions::table.filter(sessions::guest_id.eq(guest_id))).execute(conn)?;

        // Fetch the updated guest.
        let guest: Guest = guests::table
            .filter(guests::id.eq(guest_id))
            .select(Guest::as_select())
            .first(conn)?;

        // Generate UUID token and insert session.
        let uuid_token = Uuid::new_v4();
        let token_str = uuid_token.to_string();
        let new_session = NewSession {
            guest_id: guest.id,
            token: token_str.clone(),
        };
        diesel::insert_into(sessions::table)
            .values(&new_session)
            .execute(conn)?;

        Ok((guest, token_str))
    })
}

/// Retrieves an active guest by their session token.
/// Validates token as UUID and returns the guest if active.
#[cfg(feature = "ssr")]
pub fn get_guest_by_token(
    conn: &mut SqliteConnection,
    token: &str,
) -> Result<Guest, diesel::result::Error> {
    // Validate token format.
    if Uuid::parse_str(token).is_err() {
        return Err(diesel::result::Error::NotFound);
    }

    let guest: Option<Guest> = sessions::table
        .filter(sessions::token.eq(token))
        .inner_join(guests::table.on(sessions::guest_id.eq(guests::id)))
        .filter(guests::is_active.eq(1i32))
        .select(Guest::as_select())
        .first::<Guest>(conn)
        .optional()?;
    guest.ok_or(diesel::result::Error::NotFound)
}

/// Retrieves all unregistered (inactive) guests.
#[cfg(feature = "ssr")]
pub fn get_all_unregistered_guests(
    conn: &mut SqliteConnection,
) -> Result<Vec<Guest>, diesel::result::Error> {
    guests::table
        .filter(guests::is_active.eq(0i32))
        .select(Guest::as_select())
        .load(conn)
}

/// Unregisters a guest, deletes sessions associated with that guest.
/// Returns number of affected rows.
#[cfg(feature = "ssr")]
pub fn unregister_guest(
    conn: &mut SqliteConnection,
    guest_id: i32,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(sessions::table.filter(sessions::guest_id.eq(guest_id))).execute(conn)?;

    diesel::update(guests::table.filter(guests::id.eq(guest_id)))
        .set(guests::is_active.eq(0i32))
        .execute(conn)
}

/// Reregisters a guest: Reactivates them, optionally changes house and character, updates registered_at,
/// deletes old session (if any), and generates a new token.
/// Returns updated guest and new token if an entry for this guest already exists, or NotFound
/// error otherwise.
#[cfg(feature = "ssr")]
pub fn reregister_guest(
    conn: &mut SqliteConnection,
    guest_id: i32,
    new_house_id: Option<i32>,
    new_character: Option<&str>,
) -> Result<(Guest, String), diesel::result::Error> {
    conn.transaction(|conn| {
        // Fetch guest entry if it exists.
        let existing_guest: Option<Guest> = guests::table
            .filter(guests::id.eq(guest_id))
            .select(Guest::as_select())
            .first::<Guest>(conn)
            .optional()?;
        let mut guest = match existing_guest {
            Some(g) => g,
            None => return Err(diesel::result::Error::NotFound),
        };

        // Update house if provided, after validating that it exists.
        if let Some(house_id) = new_house_id {
            let house_exists: i64 = houses::table
                .filter(houses::id.eq(house_id))
                .count()
                .get_result(conn)?;
            if house_exists == 0 {
                return Err(diesel::result::Error::NotFound);
            }
            diesel::update(guests::table.filter(guests::id.eq(guest_id)))
                .set(guests::house_id.eq(Some(house_id)))
                .execute(conn)?;
            guest.house_id = Some(house_id);
        }

        // Update the character if provided.
        if let Some(char_name) = new_character {
            diesel::update(guests::table.filter(guests::id.eq(guest_id)))
                .set(guests::character.eq(Some(char_name.to_string())))
                .execute(conn)?;
            guest.character = Some(char_name.to_string());
        }

        // Reactivate and update registered_at to now.
        let now = Utc::now().naive_utc();
        diesel::update(guests::table.filter(guests::id.eq(guest_id)))
            .set((
                guests::is_active.eq(1i32),
                guests::registered_at.eq(Some(now)),
            ))
            .execute(conn)?;

        // Delete old session.
        diesel::delete(sessions::table.filter(sessions::guest_id.eq(guest_id))).execute(conn)?;

        // Generate new token and session.
        let uuid_token = Uuid::new_v4();
        let token_str = uuid_token.to_string();
        let new_session = NewSession {
            guest_id,
            token: token_str.clone(),
        };
        diesel::insert_into(sessions::table)
            .values(&new_session)
            .execute(conn)?;

        // Refetch updated guest.
        let updated_guest: Guest = guests::table
            .filter(guests::id.eq(guest_id))
            .select(Guest::as_select())
            .first(conn)?;

        Ok((updated_guest, token_str))
    })
}

/// Awards or deducts points to a guest. Updates both the guest's personal score and the house
/// score, and logs the award.
#[cfg(feature = "ssr")]
pub fn award_points_to_guest(
    conn: &mut SqliteConnection,
    guest_id: i32,
    amount: i32,
    reason: &str,
) -> Result<PointAward, diesel::result::Error> {
    conn.transaction(|conn| {
        // Fetch the active guest first.
        let guest: Guest = guests::table
            .filter(guests::id.eq(guest_id))
            .filter(guests::is_active.eq(1i32))
            .select(Guest::as_select())
            .first(conn)?;

        // Ensure the guest has a house assigned (for active guests).
        let house_id = guest.house_id.ok_or(diesel::result::Error::NotFound)?;

        // Fetch the house.
        let house: House = houses::table
            .filter(houses::id.eq(house_id))
            .select(House::as_select())
            .first(conn)?;

        // Update the guest's personal score.
        diesel::update(guests::table.filter(guests::id.eq(guest_id)))
            .set(guests::personal_score.eq(guest.personal_score + amount))
            .execute(conn)?;

        // Update the house score.
        diesel::update(houses::table.filter(houses::id.eq(house.id)))
            .set(houses::score.eq(house.score + amount))
            .execute(conn)?;

        // Log the award.
        let new_award = NewPointAward {
            guest_id: Some(guest_id),
            house_id: None,
            amount,
            reason: reason.to_string(),
            awarded_at: Utc::now().naive_utc(),
        };
        diesel::insert_into(point_awards::table)
            .values(&new_award)
            .get_result(conn)
    })
}

/// Awards or deducts points to a house and logs the award.
#[cfg(feature = "ssr")]
pub fn award_points_to_house(
    conn: &mut SqliteConnection,
    house_id: i32,
    amount: i32,
    reason: &str,
) -> Result<PointAward, diesel::result::Error> {
    conn.transaction(|conn| {
        let house: House = houses::table
            .filter(houses::id.eq(house_id))
            .select(House::as_select())
            .first(conn)?;

        diesel::update(houses::table.filter(houses::id.eq(house_id)))
            .set(houses::score.eq(house.score + amount))
            .execute(conn)?;

        let new_award = NewPointAward {
            guest_id: None,
            house_id: Some(house_id),
            amount,
            reason: reason.to_string(),
            awarded_at: Utc::now().naive_utc(),
        };
        diesel::insert_into(point_awards::table)
            .values(&new_award)
            .get_result(conn)
    })
}

/// Creates an admin session and returns the token.
#[cfg(feature = "ssr")]
pub fn create_admin_session(conn: &mut SqliteConnection) -> Result<String, diesel::result::Error> {
    let uuid_token = Uuid::new_v4();
    let token_str = uuid_token.to_string();
    let new_session = NewAdminSession {
        token: token_str.clone(),
    };
    diesel::insert_into(admin_sessions::table)
        .values(&new_session)
        .execute(conn)?;
    Ok(token_str)
}

/// Validates an admin token. Returns true if the provided token exists in the admin_sessions
/// table.
#[cfg(feature = "ssr")]
pub fn validate_admin_token(
    conn: &mut SqliteConnection,
    token: &str,
) -> Result<bool, diesel::result::Error> {
    if Uuid::parse_str(token).is_err() {
        return Ok(false);
    }
    let count: i64 = admin_sessions::table
        .filter(admin_sessions::token.eq(token))
        .count()
        .get_result(conn)?;
    Ok(count > 0)
}

/// Returns the session token for a specific guest, if it exists.
#[cfg(feature = "ssr")]
pub fn get_guest_token(
    conn: &mut SqliteConnection,
    guest_id: i32,
) -> Result<Option<String>, diesel::result::Error> {
    sessions::table
        .filter(sessions::guest_id.eq(guest_id))
        .select(sessions::token)
        .first(conn)
        .optional()
}

/// Returns all point awards with guest and/or house names, in reverse chronological order.
#[cfg(feature = "ssr")]
pub fn get_all_point_awards(
    conn: &mut SqliteConnection,
) -> Result<Vec<PointAwardLog>, diesel::result::Error> {
    point_awards::table
        .left_join(guests::table.on(point_awards::guest_id.eq(guests::id.nullable())))
        .left_join(houses::table.on(point_awards::house_id.eq(houses::id.nullable())))
        .select((
            point_awards::id,
            guests::name.nullable(),
            houses::name.nullable(),
            point_awards::amount,
            point_awards::reason,
            point_awards::awarded_at,
        ))
        .order(point_awards::awarded_at.desc())
        .load(conn)
}

/// Fetches the crossword completion progress for all houses.
/// Returns a 4x7 boolean matrix: rows = houses (0=Gryffindor/id1, 1=Hufflepuff/id2, 2=Ravenclaw/id3, 3=Slytherin/id4),
/// columns = words (0-6). true if house has completed that word.
#[cfg(feature = "ssr")]
pub fn get_house_crossword_progress(
    conn: &mut SqliteConnection,
) -> Result<Vec<Vec<bool>>, diesel::result::Error> {
    let completions: Vec<HouseCrosswordCompletion> = house_crossword_completions::table
        .inner_join(houses::table.on(house_crossword_completions::house_id.eq(houses::id)))
        .select(HouseCrosswordCompletion::as_select())
        .load(conn)?;

    let mut matrix: Vec<Vec<bool>> = vec![vec![false; 7]; 4];

    for completion in completions {
        let house_idx = match completion.house_id {
            1 => 0,
            2 => 1,
            3 => 2,
            4 => 3,
            _ => continue,
        };
        let word_idx = completion.word_index as usize;
        if word_idx < 7 {
            matrix[house_idx][word_idx] = true;
        }
    }

    Ok(matrix)
}

/// Fetches all houses.
#[cfg(feature = "ssr")]
pub fn get_all_houses(conn: &mut SqliteConnection) -> Result<Vec<House>, diesel::result::Error> {
    houses::table
        .order(houses::name)
        .select(House::as_select())
        .load(conn)
}

/// Fetches a guest's details. Assumes the guest is active and has been sorted already. Returns an
/// error otherwise.
#[cfg(feature = "ssr")]
pub fn get_guest_details(
    conn: &mut SqliteConnection,
    guest_id: i32,
) -> Result<(Guest, House), diesel::result::Error> {
    // Fetch the active guest first.
    let guest: Guest = guests::table
        .filter(guests::id.eq(guest_id))
        .filter(guests::is_active.eq(1i32))
        .select(Guest::as_select())
        .first(conn)?;

    // Ensure the guest has a house assigned.
    let house_id = guest.house_id.ok_or(diesel::result::Error::NotFound)?;

    // Fetch the house.
    let house: House = houses::table
        .filter(houses::id.eq(house_id))
        .select(House::as_select())
        .first(conn)?;

    Ok((guest, house))
}

/// Retrieves all active guests.
#[cfg(feature = "ssr")]
pub fn get_all_active_guests(
    conn: &mut SqliteConnection,
) -> Result<Vec<Guest>, diesel::result::Error> {
    guests::table
        .filter(guests::is_active.eq(1i32))
        .select(Guest::as_select())
        .load(conn)
}

/// Resets the entire database to its initial state.
#[cfg(feature = "ssr")]
pub fn reset_database(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    conn.transaction(|conn| {
        // Delete all sessions (guest and admin).
        diesel::delete(sessions::table).execute(conn)?;
        diesel::delete(admin_sessions::table).execute(conn)?;

        // Delete all point awards.
        diesel::delete(point_awards::table).execute(conn)?;

        // Delete all guest crossword states.
        diesel::delete(crossword_states::table).execute(conn)?;

        // Delete all house crossword completion entries.
        diesel::delete(house_crossword_completions::table).execute(conn)?;

        // Delete all votes.
        diesel::delete(votes::table).execute(conn)?;

        // Reset voting status.
        diesel::update(voting_status::table)
            .set((
                voting_status::is_open.eq(0i32),
                voting_status::opened_at.eq::<Option<chrono::NaiveDateTime>>(None),
                voting_status::closed_at.eq::<Option<chrono::NaiveDateTime>>(None),
            ))
            .execute(conn)?;

        // Reset all guests.
        diesel::update(guests::table)
            .set((
                guests::is_active.eq(0i32),
                guests::personal_score.eq(0i32),
                guests::house_id.eq(None::<i32>),
                guests::registered_at.eq(None::<chrono::NaiveDateTime>),
                guests::character.eq(None::<String>),
            ))
            .execute(conn)?;

        // Reset all house scores to zero.
        diesel::update(houses::table)
            .set(houses::score.eq(0i32))
            .execute(conn)?;

        Ok(())
    })
}

#[derive(Clone, Copy, Debug)]
enum Direction {
    Across,
    Down,
}

#[derive(Clone, Debug)]
struct WordDef {
    start_row: usize,
    start_col: usize,
    len: usize,
    dir: Direction,
    answer: &'static str,
    reveal_text: &'static str,
}

const CROSSWORD_DEFS: &[WordDef] = &[
    WordDef {
        start_row: 1,
        start_col: 1,
        len: 5,
        dir: Direction::Across,
        answer: "WINKY",
        reveal_text: "Behind a door where secrets sleep,\nI slither low, my watch I keep.\nNo voice, no spell, just breath and skin,\nThe darkness stirs, I wait within.",
    },
    WordDef {
        start_row: 6,
        start_col: 0,
        len: 12,
        dir: Direction::Across,
        answer: "EXPELLIARMUS",
        reveal_text: "Where portraits purr in rose-tinted frame,\nI nest in her china, igniting no flame.\nEmblem of lineage, cold and entwined,\nI whisper old venom, twisting the mind.",
    },
    WordDef {
        start_row: 2,
        start_col: 0,
        len: 10,
        dir: Direction::Down,
        answer: "DISSENDIUM",
        reveal_text: "With lemon drops and half-moon gaze,\nI unravel riddles through misty haze.\nFrom elder's core, my power flows,\nShepherding souls where the wild wind blows.",
    },
    WordDef {
        start_row: 0,
        start_col: 3,
        len: 8,
        dir: Direction::Down,
        answer: "SNUFFLES",
        reveal_text: "Once a token of toil and truth,\nNow a prison to deathless youth.\nGold surrounds me, bright and deep,\nYet secrets foul within me sleep.",
    },
    WordDef {
        start_row: 5,
        start_col: 6,
        len: 10,
        dir: Direction::Down,
        answer: "SIRCADOGAN",
        reveal_text: "Among the brave, I should not be,\nYet here I wait, in secrecy.\nMy pages whisper lies and lore,\nTo open hearts - and something more.",
    },
    WordDef {
        start_row: 3,
        start_col: 8,
        len: 9,
        dir: Direction::Down,
        answer: "BOARHOUND",
        reveal_text: "At the threshold where paths align,\nCloak, wand, and stone combine.\nThrough death I passed, through love restored,\nNow hang I here at fate's own door.",
    },
    WordDef {
        start_row: 1,
        start_col: 10,
        len: 7,
        dir: Direction::Down,
        answer: "IGNOTUS",
        reveal_text: "\"Wit beyond measure\" once was prized,\nNow in your clutter, undisguised.\nAmong the things you cast aside,\nThe clever crown still tries to hide.",
    },
];

/// Fetches the crossword state for a guest, or inserts an empty one if it doesn't exist, and
/// returns it.
#[cfg(feature = "ssr")]
pub fn get_or_init_crossword_state(
    conn: &mut SqliteConnection,
    guest_id: i32,
) -> Result<CrosswordState, diesel::result::Error> {
    let existing: Option<DbCrosswordState> = crossword_states::table
        .filter(crossword_states::guest_id.eq(guest_id))
        .first(conn)
        .optional()?;

    match existing {
        Some(db_state) => Ok(db_state.state.into()),
        None => {
            let initial_state = CrosswordState::new_full_grid(vec![vec![None; 12]; 15], [false; 7]);
            let new_db_state = NewDbCrosswordState {
                guest_id,
                state: initial_state.clone().into(),
                updated_at: chrono::Utc::now().naive_utc(),
            };
            diesel::insert_into(crossword_states::table)
                .values(&new_db_state)
                .execute(conn)?;
            Ok(initial_state)
        }
    }
}

/// Updates the crossword state for a guest. Replaces the entire row in the database.
/// Additionally, checks for new word completions by this guest, and awards house points if it's
/// the house's first completion of that word. As a result of a first time completion, if all 7
/// words are now complete by the house, awards an additional bonus.
#[cfg(feature = "ssr")]
pub fn update_crossword_state(
    conn: &mut SqliteConnection,
    guest_id: i32,
    new_state: &CrosswordState,
) -> Result<(), diesel::result::Error> {
    conn.transaction(|conn| {
        // Getch the guest to get house_id.
        let guest: Guest = guests::table
            .filter(guests::id.eq(guest_id))
            .filter(guests::is_active.eq(1i32))
            .select(Guest::as_select())
            .first(conn)?;
        let house_id = guest.house_id.ok_or(diesel::result::Error::NotFound)?;

        // Fetch the old state to compare completions.
        let old_db_state: Option<DbCrosswordState> = crossword_states::table
            .filter(crossword_states::guest_id.eq(guest_id))
            .first(conn)
            .optional()?;
        let old_completions = match old_db_state {
            Some(old) => CrosswordState::from(old.state.clone()).completions,
            None => [false; 7],
        };

        // Query the house's initial completion count before any inserts.
        let initial_count: i64 = house_crossword_completions::table
            .filter(house_crossword_completions::house_id.eq(house_id))
            .count()
            .get_result(conn)?;

        // Check for new completions and award points if first for the house. Track any new
        // insertions.
        let mut new_inserts_count = 0;
        for i in 0..7 {
            if !old_completions[i] && new_state.completions[i] {
                // This guest just completed word i.
                if !house_has_completed_word(conn, house_id, i as i32)? {
                    // First time for for the house; award 5 points and mark completed.
                    award_points_to_house(
                        conn,
                        house_id,
                        5,
                        &format!("Crossword word {} completed by house", i),
                    )?;
                    insert_house_word_completion(conn, house_id, i as i32)?;
                    new_inserts_count += 1;
                }
            }
        }

        // Check if this update caused the house to reach all 7 completions.
        let effective_final_count = initial_count + new_inserts_count as i64;
        if effective_final_count == 7 {
            award_points_to_house(conn, house_id, 15, "Crossword completion bonus")?;
        }

        // Replace the state in DB.
        diesel::delete(crossword_states::table.filter(crossword_states::guest_id.eq(guest_id)))
            .execute(conn)?;
        let db_state = NewDbCrosswordState {
            guest_id,
            state: new_state.clone().into(),
            updated_at: Utc::now().naive_utc(),
        };
        diesel::insert_into(crossword_states::table)
            .values(&db_state)
            .execute(conn)?;

        Ok(())
    })
}

/// Returs true if a house has already completed a specific crossword word.
#[cfg(feature = "ssr")]
pub fn house_has_completed_word(
    conn: &mut SqliteConnection,
    house_id: i32,
    word_index: i32,
) -> Result<bool, diesel::result::Error> {
    let count: i64 = house_crossword_completions::table
        .filter(
            house_crossword_completions::house_id
                .eq(house_id)
                .and(house_crossword_completions::word_index.eq(word_index)),
        )
        .count()
        .get_result(conn)?;
    Ok(count > 0)
}

/// Marks a house as having completed a specific crossword word.
#[cfg(feature = "ssr")]
pub fn insert_house_word_completion(
    conn: &mut SqliteConnection,
    house_id: i32,
    word_index: i32,
) -> Result<(), diesel::result::Error> {
    let new_completion = NewHouseCrosswordCompletion {
        house_id,
        word_index,
    };
    diesel::insert_into(house_crossword_completions::table)
        .values(&new_completion)
        .execute(conn)?;
    Ok(())
}

/// Initializes the voting status table with a singleton row.
#[cfg(feature = "ssr")]
pub fn init_voting_status(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    let count: i64 = voting_status::table.count().get_result(conn)?;
    if count == 0 {
        let new_status = NewVotingStatus {
            is_open: 0,
            opened_at: None,
            closed_at: None,
        };
        diesel::insert_into(voting_status::table)
            .values(&new_status)
            .execute(conn)?;
    }
    Ok(())
}

/// Returns true if voting is open, false otherwise.
#[cfg(feature = "ssr")]
pub fn voting_is_open(conn: &mut SqliteConnection) -> Result<bool, diesel::result::Error> {
    let status: Option<VotingStatus> = voting_status::table.first(conn).optional()?;
    Ok(status.map_or(false, |s| s.is_open == 1))
}

#[cfg(feature = "ssr")]
pub fn open_voting(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    conn.transaction(|conn| {
        let now = Utc::now().naive_utc();
        diesel::update(voting_status::table)
            .set((
                voting_status::is_open.eq(1i32),
                voting_status::opened_at.eq(Some(now)),
                voting_status::closed_at.eq::<Option<chrono::NaiveDateTime>>(None),
            ))
            .execute(conn)?;
        Ok(())
    })
}

#[cfg(feature = "ssr")]
pub fn close_voting(conn: &mut SqliteConnection) -> Result<RcvResult, diesel::result::Error> {
    conn.transaction(|conn| {
        let now = Utc::now().naive_utc();
        diesel::update(voting_status::table)
            .set((
                voting_status::is_open.eq(0i32),
                voting_status::closed_at.eq(Some(now)),
            ))
            .execute(conn)?;

        get_rcv_result(conn)
    })
}

#[cfg(feature = "ssr")]
pub fn submit_vote(
    conn: &mut SqliteConnection,
    voter_id: i32,
    first: i32,
    second: i32,
    third: i32,
) -> Result<(), diesel::result::Error> {
    conn.transaction(|conn| {
        if !voting_is_open(conn)? {
            return Err(diesel::result::Error::QueryBuilderError(Box::new(
                IoError::new(ErrorKind::Other, "Voting is not open"),
            )));
        }

        let voter_active: i64 = guests::table
            .filter(guests::id.eq(voter_id).and(guests::is_active.eq(1i32)))
            .count()
            .get_result(conn)?;
        if voter_active == 0 {
            return Err(diesel::result::Error::NotFound);
        }

        let choices = [first, second, third];
        let mut choice_set = HashSet::new();
        for &choice_id in &choices {
            if choice_id == voter_id {
                return Err(diesel::result::Error::QueryBuilderError(Box::new(
                    IoError::new(ErrorKind::Other, "Cannot vote for self"),
                )));
            }
            if !choice_set.insert(choice_id) {
                return Err(diesel::result::Error::QueryBuilderError(Box::new(
                    IoError::new(ErrorKind::Other, "Choices must be unique"),
                )));
            }
            let active: i64 = guests::table
                .filter(guests::id.eq(choice_id).and(guests::is_active.eq(1i32)))
                .count()
                .get_result(conn)?;
            if active == 0 {
                return Err(diesel::result::Error::NotFound);
            }
        }

        diesel::delete(votes::table.filter(votes::voter_id.eq(voter_id))).execute(conn)?;

        let new_vote = NewVote {
            voter_id,
            first_choice_id: first,
            second_choice_id: second,
            third_choice_id: third,
            submitted_at: Utc::now().naive_utc(),
        };
        diesel::insert_into(votes::table)
            .values(&new_vote)
            .execute(conn)?;

        Ok(())
    })
}

#[cfg(feature = "ssr")]
pub fn has_voted(
    conn: &mut SqliteConnection,
    voter_id: i32,
) -> Result<bool, diesel::result::Error> {
    let count: i64 = votes::table
        .filter(votes::voter_id.eq(voter_id))
        .count()
        .get_result(conn)?;
    Ok(count > 0)
}

#[cfg(feature = "ssr")]
pub fn get_user_vote(
    conn: &mut SqliteConnection,
    user_id: i32,
) -> Result<Option<(Guest, Guest, Guest)>, diesel::result::Error> {
    let vote: Option<Vote> = votes::table
        .filter(votes::voter_id.eq(user_id))
        .first(conn)
        .optional()?;

    match vote {
        Some(v) => {
            let first: Guest = guests::table
                .filter(guests::id.eq(v.first_choice_id))
                .first(conn)?;
            let second: Guest = guests::table
                .filter(guests::id.eq(v.second_choice_id))
                .first(conn)?;
            let third: Guest = guests::table
                .filter(guests::id.eq(v.third_choice_id))
                .first(conn)?;

            Ok(Some((first, second, third)))
        }
        None => Ok(None),
    }
}

#[cfg(feature = "ssr")]
pub fn get_all_votes(conn: &mut SqliteConnection) -> Result<Vec<Vote>, diesel::result::Error> {
    votes::table.select(Vote::as_select()).load(conn)
}

#[cfg(feature = "ssr")]
pub fn reset_votes(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    diesel::delete(votes::table).execute(conn)?;
    Ok(())
}

/// Returns the tuple (votes submitted, active guests).
#[cfg(feature = "ssr")]
pub fn get_voting_stats(conn: &mut SqliteConnection) -> Result<(i64, i64), diesel::result::Error> {
    let vote_count: i64 = votes::table.count().get_result(conn)?;
    let active_count: i64 = guests::table
        .filter(guests::is_active.eq(1i32))
        .count()
        .get_result(conn)?;
    Ok((vote_count, active_count))
}

#[cfg(feature = "ssr")]
pub fn compute_rcv(votes: &[Vote], candidates: &[i32]) -> RcvResult {
    if candidates.is_empty() {
        return RcvResult {
            winner_id: None,
            rounds: vec![],
        };
    }

    let mut active_candidates: HashSet<i32> = candidates.iter().cloned().collect();
    let mut active_ballots: Vec<&Vote> = votes.iter().collect();
    let mut rounds = vec![];

    let mut round_number = 1;
    while !active_candidates.is_empty() {
        // Step 1: Tally all active votes.
        let mut tallies = HashMap::<i32, i32>::new();
        for vote in &active_ballots {
            if active_candidates.contains(&vote.first_choice_id) {
                *tallies.entry(vote.first_choice_id).or_insert(0) += 1;
            } else if active_candidates.contains(&vote.second_choice_id) {
                *tallies.entry(vote.second_choice_id).or_insert(0) += 1;
            } else if active_candidates.contains(&vote.third_choice_id) {
                *tallies.entry(vote.third_choice_id).or_insert(0) += 1;
            }
        }

        // Prepare round tallies and sort them descending.
        let mut round_tallies: Vec<(i32, i32)> = active_candidates
            .iter()
            .map(|&c| (c, tallies.get(&c).copied().unwrap_or(0)))
            .collect();
        round_tallies.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        rounds.push(RcvRound {
            round_number: round_number,
            tallies: round_tallies.clone(),
            eliminated: vec![],
            winner: None,
        });

        // Step 2: Check for majority on non-discarded ballots.
        let total_ballots = active_ballots.len() as i32;
        let majority_threshold = if total_ballots > 0 {
            ((total_ballots as f64 * 0.5).ceil() as i32).max(1)
        } else {
            0
        };
        // There's a subtle edge case here - two candidates can have equal votes and both have the
        // majority (eg. 3 votes each among 6 active ballots). So we want to check that a candidate
        // has the majority _and_ the clear lead before declaring a winner.
        let top_count = round_tallies.first().map(|(_, count)| *count).unwrap_or(0);
        let is_clear_top = round_tallies.len() < 2 || round_tallies[1].1 < top_count;
        if top_count >= majority_threshold && is_clear_top {
            if let Some((winner_id, _)) = round_tallies.first() {
                rounds.last_mut().unwrap().winner = Some(*winner_id);
                return RcvResult {
                    winner_id: Some(*winner_id),
                    rounds,
                };
            }
        }

        // Step 3: No majority - eliminate candidates with least votes, and eliminate ballots that
        // don't contain at least one active candidate.
        let min_votes = round_tallies.last().map(|(_, count)| *count).unwrap_or(0);
        let to_eliminate: Vec<i32> = round_tallies
            .iter()
            .filter(|&(_, count)| *count == min_votes)
            .map(|&(id, _)| id)
            .collect();

        for &id in &to_eliminate {
            active_candidates.remove(&id);
        }
        rounds.last_mut().unwrap().eliminated = to_eliminate;

        active_ballots.retain(|vote| {
            active_candidates.contains(&vote.first_choice_id)
                || active_candidates.contains(&vote.second_choice_id)
                || active_candidates.contains(&vote.third_choice_id)
        });

        round_number += 1;
    }

    RcvResult {
        winner_id: None,
        rounds: rounds,
    }
}

#[cfg(feature = "ssr")]
pub fn get_rcv_result(conn: &mut SqliteConnection) -> Result<RcvResult, diesel::result::Error> {
    if voting_is_open(conn)? {
        return Err(diesel::result::Error::QueryBuilderError(Box::new(
            IoError::new(
                ErrorKind::Other,
                "RCV computation unavailable: voting is still open",
            ),
        )));
    }

    let votes: Vec<Vote> = get_all_votes(conn)?;
    let candidates: Vec<i32> = get_all_active_guests(conn)?
        .into_iter()
        .map(|g| g.id)
        .collect();

    Ok(compute_rcv(&votes, &candidates))
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use crate::has_voted;
    use crate::model::{AdminSession, NewGuest, Vote};
    use crate::schema::houses::dsl::*;
    use chrono::Utc;

    // Helper to run a test in a transaction. This always rolls back the transaction at the end of
    // the test to maintain a clean slate in the database.
    fn run_test_in_transaction<F>(test_fn: F)
    where
        F: FnOnce(&mut SqliteConnection) -> Result<(), diesel::result::Error>,
    {
        let mut conn = establish_connection();
        let _result: Result<(), diesel::result::Error> = conn.transaction(|conn| {
            // Run the test. Propagate real errors.
            test_fn(conn)?;
            // Force rollback on test success by returning an error.
            Err(diesel::result::Error::RollbackTransaction)
        });
        // Ignore the returned error. If the test failed, we would've already panicked.
    }

    #[test]
    fn test_connection() {
        run_test_in_transaction(|conn| {
            let count: i64 = houses.count().get_result(conn).expect("Query failed");
            assert_eq!(count, 4);

            Ok(())
        });
    }

    #[test]
    fn test_register_guest() {
        run_test_in_transaction(|conn| {
            // First, insert an inactive guest for testing (mimicking prepopulation).
            let new_inactive = NewGuest {
                name: "Test Guest",
                house_id: None,
                character: None,
                registered_at: None,
            };
            let inserted_id: i32 = diesel::insert_into(guests::table)
                .values(&new_inactive)
                .returning(guests::id)
                .get_result(conn)?;

            // Verify initially no registered at.
            let initial_guest: Guest = guests::table
                .filter(guests::id.eq(inserted_id))
                .select(Guest::as_select())
                .first(conn)?;
            assert!(initial_guest.registered_at.is_none());

            // Now register.
            let (guest, token) = register_guest(conn, inserted_id, Some(1i32), "Harry Potter")?;
            assert_eq!(guest.id, inserted_id);
            assert_eq!(guest.name, "Test Guest");
            assert_eq!(guest.house_id, Some(1));
            assert_eq!(guest.character, Some("Harry Potter".to_string()));
            assert_eq!(guest.is_active, 1);
            assert!(guest.registered_at.is_some());
            assert!(guest.registered_at.unwrap().and_utc().timestamp() > 0);
            assert!(Uuid::parse_str(&token).is_ok());

            // Verify the session exists.
            let session_count: i64 = sessions::table
                .filter(
                    sessions::guest_id
                        .eq(inserted_id)
                        .and(sessions::token.eq(&token)),
                )
                .count()
                .get_result(conn)?;
            assert_eq!(session_count, 1);

            // Try registering again (should fail).
            let err = register_guest(conn, inserted_id, Some(2i32), "Hannah Abbott")
                .expect_err("Should fail as already active");
            assert!(matches!(err, diesel::result::Error::QueryBuilderError(_)));

            // Try non-existent guest.
            let err = register_guest(conn, 999, Some(1i32), "Ron Weasley")
                .expect_err("Should fail as non-existent guest");
            assert!(matches!(err, diesel::result::Error::NotFound));

            Ok(())
        });
    }

    #[test]
    fn test_get_guest_by_token() {
        run_test_in_transaction(|conn| {
            // Insert inactive guest.
            let inserted_id: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Token Guest",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            // Register a guest.
            let (guest, token) = register_guest(conn, inserted_id, Some(3i32), "Padma Patil")
                .expect("Failed to register guest");

            // Get by token.
            let fetched: Guest = get_guest_by_token(conn, &token).expect("Failed to fetch guest");
            assert_eq!(fetched.id, guest.id);
            assert_eq!(fetched.name, "Token Guest");
            assert_eq!(fetched.is_active, 1i32);

            // Invalid token.
            assert!(get_guest_by_token(conn, "invalid-uuid").is_err());

            Ok(())
        });
    }

    #[test]
    fn test_unregister_guest() {
        run_test_in_transaction(|conn| {
            // Insert inactive guest.
            let inserted_id: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Unregister Guest",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            // Register a guest.
            let (guest, _) = register_guest(conn, inserted_id, Some(3i32), "Terry Boot")
                .expect("Failed to register guest");

            // Unregister the guest.
            let affected = unregister_guest(conn, guest.id).expect("Failed to unregister guest");
            assert_eq!(affected, 1);

            // Verify that the guest is inactive and their session was deleted.
            let updated_guest: Option<Guest> = guests::table
                .filter(guests::id.eq(guest.id))
                .select(Guest::as_select())
                .first(conn)
                .optional()
                .expect("Failed to fetch guest");
            assert_eq!(updated_guest.expect("Guest not found").is_active, 0i32);

            let session_count: i64 = sessions::table
                .filter(sessions::guest_id.eq(guest.id))
                .count()
                .get_result(conn)
                .expect("Failed to count sessions");
            assert_eq!(session_count, 0);

            Ok(())
        });
    }

    #[test]
    fn test_unregister_nonexistent_guest() {
        run_test_in_transaction(|conn| {
            // Unregistering a non-existent ID should just be a no-op.
            let result = unregister_guest(conn, 42);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 0);

            Ok(())
        });
    }

    #[test]
    fn test_reregister_guest() {
        run_test_in_transaction(|conn| {
            // Insert inactive guest.
            let inserted_id: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Reregister Guest",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            // Register, then unregister a guest.
            let (guest, _) = register_guest(conn, inserted_id, Some(4i32), "Draco Malfoy")
                .expect("Failed to register guest");
            unregister_guest(conn, guest.id).expect("Failed to unregister guest");

            // Reregister with new house.
            let (reregistered, new_token) =
                reregister_guest(conn, guest.id, Some(1), Some("Ron Weasley"))
                    .expect("Failed to reregister guest");
            assert_eq!(reregistered.id, guest.id);
            assert_eq!(reregistered.house_id, Some(1));
            assert_eq!(reregistered.is_active, 1i32);
            assert_eq!(reregistered.character, Some("Ron Weasley".to_string()));
            assert!(!new_token.is_empty());
            assert!(Uuid::parse_str(&new_token).is_ok());

            // Verify new session.
            let session_count: i64 = sessions::table
                .filter(sessions::token.eq(&new_token))
                .count()
                .get_result(conn)
                .expect("Failed to count sessions");
            assert_eq!(session_count, 1);

            // Reregister without house change, verify that house id remains the same but session token
            // changes.
            let (no_change, no_change_token) =
                reregister_guest(conn, guest.id, None, Some("Hermione Granger"))
                    .expect("Failed to reregister guest");
            assert_eq!(no_change.house_id, Some(1));
            assert_eq!(no_change.character, Some("Hermione Granger".to_string()));
            assert_ne!(no_change_token, new_token);

            // Reregister a guest that doesn't exist, verify that an error is returned.
            assert!(reregister_guest(conn, 42, None, None).is_err());

            // Reregister a guest with a house that doesn't exist, verify that an error is returned.
            assert!(reregister_guest(conn, guest.id, Some(69), None).is_err());

            Ok(())
        });
    }

    #[test]
    fn test_get_all_houses() {
        run_test_in_transaction(|conn| {
            // Verify that we can read all 4 houses.
            let all_houses = get_all_houses(conn)?;
            assert_eq!(all_houses.len(), 4);
            assert!(all_houses.iter().find(|h| h.name == "Gryffindor").is_some());
            assert!(all_houses.iter().find(|h| h.name == "Hufflepuff").is_some());
            assert!(all_houses.iter().find(|h| h.name == "Ravenclaw").is_some());
            assert!(all_houses.iter().find(|h| h.name == "Slytherin").is_some());

            Ok(())
        });
    }

    #[test]
    fn test_get_guest_details() {
        run_test_in_transaction(|conn| {
            // Insert inactive guest.
            let inserted_id: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            // Register a guest with Gryffindor.
            let (guest, _) = register_guest(conn, inserted_id, Some(1i32), "Hagrid")?;
            let guest_id = guest.id;

            // Read the guest details and verify that they are correct.
            let (guest, house) = get_guest_details(conn, guest_id)?;
            assert_eq!(guest.id, guest_id);
            assert_eq!(guest.name, "Guest");
            assert_eq!(guest.character, Some("Hagrid".to_string()));
            assert_eq!(house.name, "Gryffindor");

            // Verify that reading a non-existent guest results in error.
            let err_nonexistent = get_guest_details(conn, 999).expect_err("Should fail");
            assert!(matches!(err_nonexistent, diesel::result::Error::NotFound));

            // Verify that reading an unregistered guest results in error.
            unregister_guest(conn, guest_id)?;
            let err_unregistered = get_guest_details(conn, guest_id).expect_err("Should fail");
            assert!(matches!(err_unregistered, diesel::result::Error::NotFound));

            Ok(())
        });
    }

    #[test]
    fn test_award_points_to_guest() {
        run_test_in_transaction(|conn| {
            // Insert some inactive guests.
            let id_1: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 1",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;
            let id_2: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 2",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;
            let id_3: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 3",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            // Register 3 guests - 2 in Gryffindor and 1 in Slytherin.
            let (lavender, _) = register_guest(conn, id_1, Some(1i32), "Lavender Brown")?;
            let (parvati, _) = register_guest(conn, id_2, Some(1i32), "Parvati Patil")?;
            let (pansy, _) = register_guest(conn, id_3, Some(4i32), "Pansy Parkinson")?;

            // Award points to first Gryffindor guest, and verify the contents of the returned value.
            let award = award_points_to_guest(conn, lavender.id, 10, "Game win")?;
            assert_eq!(award.amount, 10);
            assert_eq!(award.reason, "Game win");
            assert_eq!(award.guest_id, Some(lavender.id));

            // Read the guest details and verify the individual and house points.
            let (lavender, gryffindor) = get_guest_details(conn, lavender.id)?;
            assert_eq!(lavender.personal_score, 10);
            assert_eq!(gryffindor.score, 10);

            // Deduct points from the same guest. Read the guest details and verify the individual
            // and house points.
            award_points_to_guest(conn, lavender.id, -5, "Penalty")?;
            let (lavender, gryffindor) = get_guest_details(conn, lavender.id)?;
            assert_eq!(lavender.personal_score, 5);
            assert_eq!(gryffindor.score, 5);

            // Award points to second Gryffindor guest. Read the guest details and verify the
            // individual and house points.
            award_points_to_guest(conn, parvati.id, 20, "Game win")?;
            let (parvati, gryffindor) = get_guest_details(conn, parvati.id)?;
            assert_eq!(parvati.personal_score, 20);
            assert_eq!(gryffindor.score, 25);

            // Award points to Slytherin guest. Read the guest details and verify the individual
            // and house points.
            award_points_to_guest(conn, pansy.id, 15, "Game win")?;
            let (pansy, slytherin) = get_guest_details(conn, pansy.id)?;
            assert_eq!(pansy.personal_score, 15);
            assert_eq!(slytherin.score, 15);

            // Award points to a non-existent guest, and verify that an error is returned.
            let err = award_points_to_guest(conn, 999, 10, "Chumma").expect_err("Should fail");
            assert!(matches!(err, diesel::result::Error::NotFound));

            Ok(())
        });
    }

    #[test]
    fn test_award_points_to_house() {
        run_test_in_transaction(|conn| {
            // Award points to Gryffindor and verify the contents of the returned value.
            let award = award_points_to_house(conn, 2, 10, "Guest earned")?;
            assert_eq!(award.amount, 10);
            assert_eq!(award.house_id, Some(2));
            assert_eq!(award.guest_id, None);

            // Award miscellaneous points to all houses.
            award_points_to_house(conn, 2, -5, "")?;
            award_points_to_house(conn, 3, 15, "")?;
            award_points_to_house(conn, 2, 25, "")?;
            award_points_to_house(conn, 4, -5, "")?;
            award_points_to_house(conn, 3, -5, "")?;

            // Verify the final tally for all houses.
            let all_houses = get_all_houses(conn)?;
            assert_eq!(
                all_houses
                    .iter()
                    .find(|h| h.id == 1)
                    .expect("Failed to find Gryffindoe")
                    .score,
                0
            );
            assert_eq!(
                all_houses
                    .iter()
                    .find(|h| h.id == 2)
                    .expect("Failed to find Hufflepuff")
                    .score,
                30
            );
            assert_eq!(
                all_houses
                    .iter()
                    .find(|h| h.id == 3)
                    .expect("Failed to find Ravenclaw")
                    .score,
                10
            );
            assert_eq!(
                all_houses
                    .iter()
                    .find(|h| h.id == 4)
                    .expect("Failed to find Slytherin")
                    .score,
                -5
            );

            let err = award_points_to_house(conn, 42, 10, "Chumma").expect_err("Should fail");
            assert!(matches!(err, diesel::result::Error::NotFound));

            Ok(())
        });
    }

    #[test]
    fn test_get_all_active_guests() {
        run_test_in_transaction(|conn| {
            // Insert some inactive guests.
            let id_1: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 1",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;
            let id_2: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 2",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;
            let _: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 3",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            let active = get_all_active_guests(conn)?;
            assert_eq!(active.len(), 0);

            // Register some guests.
            register_guest(conn, id_1, Some(1i32), "Seamus Finnigan")?;
            register_guest(conn, id_2, Some(2i32), "Justin Finch-Fletchley")?;

            let active = get_all_active_guests(conn)?;
            assert_eq!(active.len(), 2);
            assert!(active.iter().any(|g| g.name == "Guest 1"));
            assert!(active.iter().any(|g| g.name == "Guest 2"));

            Ok(())
        });
    }

    #[test]
    fn test_reset_database() {
        run_test_in_transaction(|conn| {
            // Insert some inactive guests.
            let id_1: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 1",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;
            let id_2: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 2",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            // Register some guests and award points.
            let (guest_1, _) = register_guest(conn, id_1, Some(1i32), "Vincent Crabbe")?;
            let (guest_2, _) = register_guest(conn, id_2, Some(2i32), "Gregory Goyle")?;
            award_points_to_guest(conn, guest_1.id, 10, "Guest 1 award")?;
            award_points_to_guest(conn, guest_2.id, 20, "Guest 2 award")?;
            award_points_to_house(conn, 1, 15, "House award")?;
            award_points_to_house(conn, 2, 5, "House award")?;

            // Verify the data exists.
            let guests_count: i64 = guests::table.count().get_result(conn)?;
            assert!(guests_count >= 2); // Account for prepopulated guests.
            let sessions_count: i64 = sessions::table.count().get_result(conn)?;
            assert_eq!(sessions_count, 2);
            let awards_count: i64 = point_awards::table.count().get_result(conn)?;
            assert_eq!(awards_count, 4);

            // Reset database.
            reset_database(conn)?;

            let guests_count: i64 = guests::table.count().get_result(conn)?;
            assert!(guests_count > 0);
            let sessions_count: i64 = sessions::table.count().get_result(conn)?;
            assert_eq!(sessions_count, 0);
            let awards_count: i64 = point_awards::table.count().get_result(conn)?;
            assert_eq!(awards_count, 0);

            Ok(())
        });
    }

    #[test]
    fn test_create_admin_session() {
        run_test_in_transaction(|conn| {
            // Create a session and verify it's inserted.
            let token = create_admin_session(conn)?;
            assert!(!token.is_empty());
            assert!(Uuid::parse_str(&token).is_ok());

            // Verify the session exists in the DB.
            let count: i64 = admin_sessions::table
                .filter(admin_sessions::token.eq(&token))
                .count()
                .get_result(conn)?;
            assert_eq!(count, 1);

            // Check created_at is not null.
            let session: AdminSession = admin_sessions::table
                .filter(admin_sessions::token.eq(&token))
                .first(conn)?;
            assert!(session.created_at.and_utc().timestamp() > 0);
            assert!(session.expires_at.is_none());

            Ok(())
        });
    }

    #[test]
    fn test_validate_admin_token_valid() {
        run_test_in_transaction(|conn| {
            // Create a session.
            let token = create_admin_session(conn)?;

            // Validate it.
            let is_valid = validate_admin_token(conn, &token)?;
            assert!(is_valid);

            Ok(())
        });
    }

    #[test]
    fn test_validate_admin_token_invalid_uuid() {
        run_test_in_transaction(|conn| {
            // Create an invalid UUID.
            let invalid_token = "not-a-uuid".to_string();
            let is_valid = validate_admin_token(conn, &invalid_token)?;
            assert!(!is_valid);

            Ok(())
        });
    }

    #[test]
    fn test_validate_admin_token_nonexistent() {
        run_test_in_transaction(|conn| {
            // Create a valid UUID that is not in the DB.
            let nonexistent_token = Uuid::new_v4().to_string();
            let is_valid = validate_admin_token(conn, &nonexistent_token)?;
            assert!(!is_valid);

            Ok(())
        });
    }

    #[test]
    fn test_get_guest_token_existing() {
        run_test_in_transaction(|conn| {
            // Insert inactive guest.
            let inserted_id: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 1",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            // Register a guest.
            let (guest, _) = register_guest(conn, inserted_id, Some(1i32), "Bill Weasley")?;

            // Get the token.
            let token_opt = get_guest_token(conn, guest.id)?;
            assert!(token_opt.is_some());
            let token = token_opt.unwrap();
            assert!(!token.is_empty());
            assert!(Uuid::parse_str(&token).is_ok());

            // Verify it's the same as in session.
            let session_token: String = sessions::table
                .filter(sessions::guest_id.eq(guest.id))
                .select(sessions::token)
                .first(conn)?;
            assert_eq!(token, session_token);

            Ok(())
        });
    }

    #[test]
    fn test_get_guest_token_nonexistent() {
        run_test_in_transaction(|conn| {
            let token_opt = get_guest_token(conn, 999)?;
            assert!(!token_opt.is_some());

            Ok(())
        });
    }

    #[test]
    fn test_get_all_point_awards_empty() {
        run_test_in_transaction(|conn| {
            let awards = get_all_point_awards(conn)?;
            assert!(awards.is_empty());

            Ok(())
        });
    }

    #[test]
    fn test_get_all_point_awards_with_guest_award() {
        run_test_in_transaction(|conn| {
            // Insert inactive guest.
            let inserted_id: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Award Guest",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            let (guest, _) = register_guest(conn, inserted_id, Some(1i32), "Neville Longbottom")?;
            let award = award_points_to_guest(conn, guest.id, 10, "No reason")?;

            let awards = get_all_point_awards(conn)?;
            assert_eq!(awards.len(), 1);
            let log_entry = &awards[0];
            assert_eq!(log_entry.id, award.id);
            assert_eq!(log_entry.guest_name, Some("Award Guest".to_string()));
            assert_eq!(log_entry.house_name, None);
            assert_eq!(log_entry.amount, 10);
            assert_eq!(log_entry.reason, "No reason".to_string());
            assert!(log_entry.awarded_at.and_utc().timestamp() > 0);

            Ok(())
        });
    }

    #[test]
    fn test_get_all_point_awards_with_house_award() {
        run_test_in_transaction(|conn| {
            let award = award_points_to_house(conn, 1, 10, "No reason")?;

            let awards = get_all_point_awards(conn)?;
            assert_eq!(awards.len(), 1);
            let log_entry = &awards[0];
            assert_eq!(log_entry.id, award.id);
            assert_eq!(log_entry.guest_name, None);
            assert_eq!(log_entry.house_name, Some("Gryffindor".to_string()));
            assert_eq!(log_entry.amount, 10);
            assert_eq!(log_entry.reason, "No reason".to_string());
            assert!(log_entry.awarded_at.and_utc().timestamp() > 0);

            Ok(())
        });
    }

    #[test]
    fn test_get_all_point_awards_multiple_ordered() {
        run_test_in_transaction(|conn| {
            // Insert some inactive guests.
            let id_1: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 1",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;
            let id_2: i32 = diesel::insert_into(guests::table)
                .values(&NewGuest {
                    name: "Guest 2",
                    house_id: None,
                    character: None,
                    registered_at: None,
                })
                .returning(guests::id)
                .get_result(conn)?;

            let (guest_1, _) = register_guest(conn, id_1, Some(1i32), "Fred Weasley")?;
            award_points_to_guest(conn, guest_1.id, 10, "First")?;
            std::thread::sleep(std::time::Duration::from_millis(1));
            award_points_to_house(conn, 4, 5, "Second")?;
            std::thread::sleep(std::time::Duration::from_millis(1));
            let (guest_2, _) = register_guest(conn, id_2, Some(3i32), "George Weasley")?;
            award_points_to_guest(conn, guest_2.id, 5, "Third")?;
            std::thread::sleep(std::time::Duration::from_millis(1));
            award_points_to_guest(conn, guest_1.id, 20, "Fourth")?;

            let awards = get_all_point_awards(conn)?;
            assert_eq!(awards.len(), 4);
            assert_eq!(awards[0].reason, "Fourth".to_string());
            assert_eq!(awards[1].reason, "Third".to_string());
            assert_eq!(awards[2].reason, "Second".to_string());
            assert_eq!(awards[3].reason, "First".to_string());

            Ok(())
        });
    }

    #[test]
    fn test_house_has_completed_word_nominal() {
        run_test_in_transaction(|conn| {
            // No record exists initially -> false.
            assert!(!house_has_completed_word(conn, 1, 0)?);

            // Insert a completion.
            insert_house_word_completion(conn, 1, 0)?;

            // Now it exists -> true.
            assert!(house_has_completed_word(conn, 1, 0)?);

            // Different word -> false.
            assert!(!house_has_completed_word(conn, 1, 1)?);

            // Different house -> false.
            assert!(!house_has_completed_word(conn, 2, 0)?);

            Ok(())
        });
    }

    #[test]
    fn test_house_has_completed_word_edge_cases() {
        run_test_in_transaction(|conn| {
            // Non-existent house id -> false (no record).
            assert!(!house_has_completed_word(conn, 999, 0)?);

            // Invalid word_index (out of 0-6 range) -> false (no record, and DB CHECK would
            // prevent insert anyway).
            assert!(!house_has_completed_word(conn, 1, -1)?);
            assert!(!house_has_completed_word(conn, 1, 7)?);

            // Valid house, valid index, but no record -> false.
            assert!(!house_has_completed_word(conn, 1, 3)?);

            Ok(())
        });
    }

    #[test]
    fn test_insert_house_word_completion_nominal() {
        run_test_in_transaction(|conn| {
            // Valid house_id, valid word_index -> succeeds.
            assert!(insert_house_word_completion(conn, 1, 2).is_ok());
            assert!(house_has_completed_word(conn, 1, 2)?);
            let completion: HouseCrosswordCompletion = house_crossword_completions::table
                .filter(
                    house_crossword_completions::house_id
                        .eq(1)
                        .and(house_crossword_completions::word_index.eq(2)),
                )
                .first(conn)?;
            assert!(completion.completed_at.and_utc().timestamp() > 0);
            assert_eq!(completion.house_id, 1);
            assert_eq!(completion.word_index, 2);

            Ok(())
        });
    }

    #[test]
    fn test_insert_house_word_completion_edge_cases() {
        run_test_in_transaction(|conn| {
            // Non-existent house_id -> fails with DatabaseError (FK constraint violation).
            let err = insert_house_word_completion(conn, 999, 0)
                .expect_err("Should fail for non-existent house");
            assert!(matches!(err, diesel::result::Error::DatabaseError { .. }));

            // Invalid word index (out of 0-6) -> fails with DatabaseError (CHECK constraint).
            let err = insert_house_word_completion(conn, 1, -1)
                .expect_err("Should fail for negative word index");
            assert!(matches!(err, diesel::result::Error::DatabaseError { .. }));

            let err = insert_house_word_completion(conn, 1, 7)
                .expect_err("Should fail for too high word index");
            assert!(matches!(err, diesel::result::Error::DatabaseError { .. }));

            // Duplicate insertion -> fails with DatabaseError (UNIQUE constraint).
            insert_house_word_completion(conn, 1, 0)?;
            let err = insert_house_word_completion(conn, 1, 0)
                .expect_err("Should fail for duplicate insertion");
            assert!(matches!(err, diesel::result::Error::DatabaseError { .. }));

            Ok(())
        });
    }

    #[test]
    fn test_get_house_crossword_progress_nominal() {
        run_test_in_transaction(|conn| {
            let matrix = get_house_crossword_progress(conn)?;
            assert_eq!(matrix.len(), 4);
            for row in &matrix {
                assert_eq!(row.len(), 7);
                assert!(row.iter().all(|&c| !c));
            }

            insert_house_word_completion(conn, 1, 0)?;
            insert_house_word_completion(conn, 2, 1)?;
            insert_house_word_completion(conn, 3, 2)?;
            insert_house_word_completion(conn, 4, 3)?;
            insert_house_word_completion(conn, 1, 4)?;

            let matrix = get_house_crossword_progress(conn)?;
            assert_eq!(
                matrix[0],
                vec![true, false, false, false, true, false, false]
            );
            assert_eq!(
                matrix[1],
                vec![false, true, false, false, false, false, false]
            );
            assert_eq!(
                matrix[2],
                vec![false, false, true, false, false, false, false]
            );
            assert_eq!(
                matrix[3],
                vec![false, false, false, true, false, false, false]
            );

            for i in 0..7i32 {
                let _ = insert_house_word_completion(conn, 1, i);
            }
            let matrix = get_house_crossword_progress(conn)?;
            assert!(matrix[0].iter().all(|&c| c));

            Ok(())
        });
    }

    #[test]
    fn test_get_house_crossword_progress_edge_cases() {
        run_test_in_transaction(|conn| {
            // Invalid house_id in completion -> ignored (matrix all false).
            // Manually insert invalid (bypasses FK for test; in prod, FK prevents).
            diesel::insert_into(house_crossword_completions::table)
                .values(&NewHouseCrosswordCompletion {
                    house_id: 5, // invalid
                    word_index: 0,
                })
                .execute(conn)?;
            let matrix = get_house_crossword_progress(conn)?;
            assert!(matrix.iter().flatten().all(|&c| !c));

            // Invalid word_index (>=7) -> ignored.
            diesel::insert_into(house_crossword_completions::table)
                .values(&NewHouseCrosswordCompletion {
                    house_id: 1,
                    word_index: 7, // invalid
                })
                .execute(conn)?;
            let matrix = get_house_crossword_progress(conn)?;
            assert!(matrix.iter().flatten().all(|&c| !c));

            // Negative word_index -> ignored.
            diesel::insert_into(house_crossword_completions::table)
                .values(&NewHouseCrosswordCompletion {
                    house_id: 1,
                    word_index: -1, // invalid
                })
                .execute(conn)?;
            let matrix = get_house_crossword_progress(conn)?;
            assert!(matrix.iter().flatten().all(|&c| !c));

            Ok(())
        });
    }

    #[test]
    fn test_init_voting_status() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn).expect("Should not fail to initialize voting_status table");
            let status: VotingStatus = voting_status::table
                .first(conn)
                .expect("Should not fail to read the first row of voting_status table");
            assert_eq!(status.id, 1);
            assert_eq!(status.is_open, 0);
            assert!(status.opened_at.is_none());
            assert!(status.closed_at.is_none());

            // No error on second call.
            init_voting_status(conn).expect("Should not fail to initialize voting_status table");

            Ok(())
        });
    }

    #[test]
    fn test_voting_is_open() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn).expect("Should not fail to initialize voting_status table");
            assert!(!voting_is_open(conn).expect("Should not fail to check if voting is open"));

            diesel::update(voting_status::table)
                .set(voting_status::is_open.eq(1i32))
                .execute(conn)
                .expect("Should not fail update voting_status table");
            assert!(voting_is_open(conn).expect("Should not fail to check if voting is open"));

            Ok(())
        });
    }

    #[test]
    fn test_open_voting() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn).expect("Should not fail to initialize voting_status table");

            open_voting(conn).expect("Should not fail to open voting");
            let status: VotingStatus = voting_status::table
                .first(conn)
                .expect("Should not fail to retrieve first row of voting_status table");
            assert_eq!(status.is_open, 1);
            assert!(status.opened_at.is_some());
            assert!(status.closed_at.is_none());

            open_voting(conn).expect("Should not fail to open voting");

            Ok(())
        });
    }

    #[test]
    fn test_close_voting_no_votes() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn).expect("Should not fail to initialize voting_status table");
            open_voting(conn).expect("Should not faile to open voting");

            let result = close_voting(conn).expect("Should not fail to close voting");
            assert_eq!(result.winner_id, None);
            assert_eq!(result.rounds.len(), 0);

            let status: VotingStatus = voting_status::table
                .first(conn)
                .expect("Should not fail to retrieve first row of voting_status table");
            assert_eq!(status.is_open, 0);
            assert!(status.closed_at.is_some());

            Ok(())
        });
    }

    #[test]
    fn test_submit_vote_valid() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn)?;
            open_voting(conn)?;

            let voter_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Voter",
                        house_id: Some(1),
                        character: Some("Voter Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_1: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 1",
                        house_id: Some(2),
                        character: Some("C1 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_2: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 2",
                        house_id: Some(3),
                        character: Some("C2 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_3: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 3",
                        house_id: Some(4),
                        character: Some("C3 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;

            submit_vote(conn, voter_id, choice_1, choice_2, choice_3).expect("Should not fail");
            let vote: Vote = votes::table.first(conn)?;
            assert_eq!(vote.voter_id, voter_id);
            assert_eq!(vote.first_choice_id, choice_1);
            assert_eq!(vote.second_choice_id, choice_2);
            assert_eq!(vote.third_choice_id, choice_3);

            // Submitting again from the same voter should overwrite.
            submit_vote(conn, voter_id, choice_2, choice_3, choice_1).expect("Should not fail");
            let vote: Vote = votes::table.first(conn)?;
            assert_eq!(vote.voter_id, voter_id);
            assert_eq!(vote.first_choice_id, choice_2);
            assert_eq!(vote.second_choice_id, choice_3);
            assert_eq!(vote.third_choice_id, choice_1);

            Ok(())
        });
    }

    #[test]
    fn test_submit_vote_invalid_self() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn).expect("Should not fail to initialize voting_status table");
            open_voting(conn).expect("Should not fail to open voting");

            let voter_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Voter",
                        house_id: Some(1),
                        character: Some("Voter Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let _choice_2_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 2",
                        house_id: Some(3),
                        character: Some("C2 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let _choice_3_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 3",
                        house_id: Some(4),
                        character: Some("C3 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;

            let err =
                submit_vote(conn, voter_id, voter_id, 2, 3).expect_err("Should fail self-vote");
            assert!(matches!(err, diesel::result::Error::QueryBuilderError(_)));
            if let diesel::result::Error::QueryBuilderError(e) = err {
                assert!(e.to_string().contains("Cannot vote for self"));
            }

            Ok(())
        });
    }

    #[test]
    fn test_submit_vote_invalid_duplicate() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn).expect("Should not fail to initialize voting_status table");
            open_voting(conn).expect("Should not fail to open voting");

            let voter_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Voter",
                        house_id: Some(1),
                        character: Some("Voter Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_2_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 2",
                        house_id: Some(3),
                        character: Some("C2 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_3_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 3",
                        house_id: Some(4),
                        character: Some("C3 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;

            let err = submit_vote(conn, voter_id, choice_2_id, choice_2_id, choice_3_id)
                .expect_err("Should fail self-vote");
            assert!(matches!(err, diesel::result::Error::QueryBuilderError(_)));
            if let diesel::result::Error::QueryBuilderError(e) = err {
                assert!(e.to_string().contains("Choices must be unique"));
            }

            Ok(())
        });
    }

    #[test]
    fn test_submit_vote_closed() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn)
                .expect("Unexpectedly failed to initialize voting_status table");

            let err = submit_vote(conn, 1, 2, 3, 4).expect_err("Should fail when voting is closed");
            assert!(matches!(err, diesel::result::Error::QueryBuilderError(_)));
            if let diesel::result::Error::QueryBuilderError(e) = err {
                assert!(e.to_string().contains("Voting is not open"));
            }

            Ok(())
        });
    }

    #[test]
    fn test_has_voted() {
        run_test_in_transaction(|conn| {
            init_voting_status(conn)
                .expect("Unexpectedly failed to initialize voting_status table");
            open_voting(conn).expect("Unexpectedly failed to open voting");

            let voter_id: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Voter",
                        house_id: Some(1),
                        character: Some("Voter Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_1: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 1",
                        house_id: Some(2),
                        character: Some("C1 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_2: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 2",
                        house_id: Some(3),
                        character: Some("C2 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;
            let choice_3: i32 = diesel::insert_into(guests::table)
                .values((
                    NewGuest {
                        name: "Choice 3",
                        house_id: Some(4),
                        character: Some("C3 Char"),
                        registered_at: Some(Utc::now().naive_utc()),
                    },
                    guests::is_active.eq(1i32),
                ))
                .returning(guests::id)
                .get_result(conn)?;

            assert!(!has_voted(conn, voter_id)
                .expect("Unexpectedly failed to check if voter has voted"));

            submit_vote(conn, voter_id, choice_1, choice_2, choice_3).expect("Should not fail");
            assert!(
                has_voted(conn, voter_id).expect("Unexpectedly failed to check if voter has voted")
            );

            Ok(())
        });
    }

    #[test]
    fn test_compute_rcv_majority_first_round() {
        // In this scenario, there are 3 candidates. 1 wins by majority in the first round.
        let vote_1 = Vote {
            id: 1,
            voter_id: 10,
            first_choice_id: 1,
            second_choice_id: 2,
            third_choice_id: 3,
            submitted_at: Utc::now().naive_utc(),
        };
        let vote_2 = Vote {
            id: 1,
            voter_id: 11,
            first_choice_id: 1,
            second_choice_id: 2,
            third_choice_id: 3,
            submitted_at: Utc::now().naive_utc(),
        };
        let vote_3 = Vote {
            id: 1,
            voter_id: 12,
            first_choice_id: 1,
            second_choice_id: 2,
            third_choice_id: 3,
            submitted_at: Utc::now().naive_utc(),
        };
        let votes = vec![vote_1, vote_2, vote_3];
        let candidates = vec![1, 2, 3];

        let result = compute_rcv(&votes, &candidates);
        assert_eq!(result.winner_id, Some(1));
        assert_eq!(result.rounds.len(), 1);
        assert_eq!(result.rounds[0].tallies, vec![(1, 3), (2, 0), (3, 0)]);
        assert!(result.rounds[0].eliminated.is_empty());
        assert_eq!(result.rounds[0].winner, Some(1));
    }

    #[test]
    fn test_compute_rcv_majority_second_round() {
        // In this scenario, there are 4 candidates. 1 starts off with a strong lead, and goes on
        // to win in the second round when 4 is eliminated and their vote goes to 1.
        let votes = vec![
            Vote {
                id: 1,
                voter_id: 10,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 2,
                voter_id: 11,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 3,
                voter_id: 12,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 4,
                voter_id: 13,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 5,
                voter_id: 14,
                first_choice_id: 2,
                second_choice_id: 1,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 6,
                voter_id: 15,
                first_choice_id: 2,
                second_choice_id: 1,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 7,
                voter_id: 16,
                first_choice_id: 3,
                second_choice_id: 2,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 8,
                voter_id: 17,
                first_choice_id: 3,
                second_choice_id: 2,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 9,
                voter_id: 18,
                first_choice_id: 4,
                second_choice_id: 1,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
        ];
        let candidates = vec![1, 2, 3, 4];

        let result = compute_rcv(&votes, &candidates);
        assert_eq!(result.winner_id, Some(1));
        assert_eq!(result.rounds.len(), 2);
        assert_eq!(
            result.rounds[0].tallies,
            vec![(1, 4), (2, 2), (3, 2), (4, 1)]
        );
        assert_eq!(result.rounds[0].eliminated, vec![4]);
        assert_eq!(result.rounds[0].winner, None);
        assert_eq!(result.rounds[1].tallies, vec![(1, 5), (2, 2), (3, 2)]);
        assert!(result.rounds[1].eliminated.is_empty());
        assert_eq!(result.rounds[1].winner, Some(1));
    }

    #[test]
    fn test_compute_rcv_majority_third_round_comeback_win() {
        // In this scenario, there are 4 candidates. 1 starts off with a strong lead, but 2
        // eventually comes back to win it by gaining the ballots of 3 and 4 when they are
        // eliminated.
        let votes = vec![
            Vote {
                id: 1,
                voter_id: 10,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 2,
                voter_id: 11,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 3,
                voter_id: 12,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 4,
                voter_id: 13,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 5,
                voter_id: 14,
                first_choice_id: 2,
                second_choice_id: 1,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 6,
                voter_id: 15,
                first_choice_id: 2,
                second_choice_id: 1,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 7,
                voter_id: 16,
                first_choice_id: 3,
                second_choice_id: 2,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 8,
                voter_id: 17,
                first_choice_id: 3,
                second_choice_id: 2,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 9,
                voter_id: 18,
                first_choice_id: 4,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
        ];
        let candidates = vec![1, 2, 3, 4];

        let result = compute_rcv(&votes, &candidates);
        assert_eq!(result.winner_id, Some(2));
        assert_eq!(result.rounds.len(), 3);
        // Round 1: 1 > 4, 2 > 2, 3 > 2, 4 > 1
        // No majority. 4 is eliminated.
        assert_eq!(
            result.rounds[0].tallies,
            vec![(1, 4), (2, 2), (3, 2), (4, 1)]
        );
        assert_eq!(result.rounds[0].eliminated, vec![4]);
        assert_eq!(result.rounds[0].winner, None);
        // Round 2: 1 > 4, 2 > 3, 3 > 2
        // No majority. 4 is eliminated.
        assert_eq!(result.rounds[1].tallies, vec![(1, 4), (2, 3), (3, 2)]);
        assert_eq!(result.rounds[1].eliminated, vec![3]);
        assert_eq!(result.rounds[1].winner, None);
        // Round 3: 1 > 4, 2 > 5
        // 2 wins by majority.
        assert_eq!(result.rounds[2].tallies, vec![(2, 5), (1, 4)]);
        assert!(result.rounds[2].eliminated.is_empty());
        assert_eq!(result.rounds[2].winner, Some(2));
    }

    #[test]
    fn test_compute_rcv_one_candidate_remaining() {
        // In this scenario, there are 4 candidates. 1 starts off with a slim lead, but 2, 3, 4 are
        // tied lowest in the first round and all get eliminated, so 1 wins by default in the
        // second round.
        let votes = vec![
            Vote {
                id: 1,
                voter_id: 10,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 2,
                voter_id: 11,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 3,
                voter_id: 12,
                first_choice_id: 2,
                second_choice_id: 3,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 4,
                voter_id: 13,
                first_choice_id: 3,
                second_choice_id: 2,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 5,
                voter_id: 14,
                first_choice_id: 4,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
        ];
        let candidates = vec![1, 2, 3, 4];

        let result = compute_rcv(&votes, &candidates);
        assert_eq!(result.winner_id, Some(1));
        assert_eq!(result.rounds.len(), 2);
        // Round 1: 1 > 2, 2 > 1, 3 > 1, 4 > 1
        // No majority. 2, 3, 4 are eliminated.
        assert_eq!(
            result.rounds[0].tallies,
            vec![(1, 2), (2, 1), (3, 1), (4, 1)]
        );
        assert_eq!(result.rounds[0].eliminated, vec![2, 3, 4]);
        assert_eq!(result.rounds[0].winner, None);
        // Round 2: 1 > 4
        // 1 is the only remaining candidate, and wins.
        assert_eq!(result.rounds[1].tallies, vec![(1, 4)]);
        assert!(result.rounds[1].eliminated.is_empty());
        assert_eq!(result.rounds[1].winner, Some(1));
    }

    #[test]
    fn test_compute_rcv_tie_first_round() {
        // In this scenario, there are 4 candidates. They all receive the same number of votes, so
        // it's a tie in the first round.
        let votes = vec![
            Vote {
                id: 1,
                voter_id: 10,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 2,
                voter_id: 11,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 3,
                voter_id: 12,
                first_choice_id: 2,
                second_choice_id: 3,
                third_choice_id: 4,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 4,
                voter_id: 13,
                first_choice_id: 2,
                second_choice_id: 3,
                third_choice_id: 4,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 5,
                voter_id: 14,
                first_choice_id: 3,
                second_choice_id: 4,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 6,
                voter_id: 15,
                first_choice_id: 3,
                second_choice_id: 4,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 7,
                voter_id: 16,
                first_choice_id: 4,
                second_choice_id: 1,
                third_choice_id: 2,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 8,
                voter_id: 17,
                first_choice_id: 4,
                second_choice_id: 1,
                third_choice_id: 2,
                submitted_at: Utc::now().naive_utc(),
            },
        ];
        let candidates = vec![1, 2, 3, 4];

        let result = compute_rcv(&votes, &candidates);
        assert_eq!(result.winner_id, None);
        assert_eq!(result.rounds.len(), 1);
        // Round 1: 1 > 2, 2 > 2, 3 > 2, 4 > 2
        // No majority. All are eliminated.
        assert_eq!(
            result.rounds[0].tallies,
            vec![(1, 2), (2, 2), (3, 2), (4, 2)]
        );
        assert_eq!(result.rounds[0].eliminated, vec![1, 2, 3, 4]);
        assert_eq!(result.rounds[0].winner, None);
    }

    #[test]
    fn test_compute_rcv_tie_multiple_rounds() {
        // In this scenario, there are 6 candidates. After 3 rounds, it's a tie.
        let votes = vec![
            Vote {
                id: 1,
                voter_id: 10,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 2,
                voter_id: 11,
                first_choice_id: 1,
                second_choice_id: 2,
                third_choice_id: 3,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 3,
                voter_id: 12,
                first_choice_id: 2,
                second_choice_id: 3,
                third_choice_id: 4,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 4,
                voter_id: 13,
                first_choice_id: 2,
                second_choice_id: 3,
                third_choice_id: 4,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 5,
                voter_id: 14,
                first_choice_id: 3,
                second_choice_id: 4,
                third_choice_id: 5,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 6,
                voter_id: 15,
                first_choice_id: 3,
                second_choice_id: 4,
                third_choice_id: 5,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 7,
                voter_id: 16,
                first_choice_id: 4,
                second_choice_id: 5,
                third_choice_id: 6,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 8,
                voter_id: 17,
                first_choice_id: 4,
                second_choice_id: 5,
                third_choice_id: 6,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 9,
                voter_id: 18,
                first_choice_id: 5,
                second_choice_id: 1,
                third_choice_id: 2,
                submitted_at: Utc::now().naive_utc(),
            },
            Vote {
                id: 10,
                voter_id: 19,
                first_choice_id: 6,
                second_choice_id: 2,
                third_choice_id: 1,
                submitted_at: Utc::now().naive_utc(),
            },
        ];
        let candidates = vec![1, 2, 3, 4, 5, 6];

        let result = compute_rcv(&votes, &candidates);
        assert_eq!(result.winner_id, None);
        assert_eq!(result.rounds.len(), 3);
        // Round 1: 1 > 2, 2 > 2, 3 > 2, 4 > 2, 5 > 1, 6 > 1
        // No majority. 5, 6 are eliminated.
        assert_eq!(
            result.rounds[0].tallies,
            vec![(1, 2), (2, 2), (3, 2), (4, 2), (5, 1), (6, 1)]
        );
        assert_eq!(result.rounds[0].eliminated, vec![5, 6]);
        assert_eq!(result.rounds[0].winner, None);
        // Round 2: 1 > 3, 2 > 3, 3 > 2, 4 > 2
        // No majority. 3, 4 are eliminated.
        assert_eq!(
            result.rounds[1].tallies,
            vec![(1, 3), (2, 3), (3, 2), (4, 2)]
        );
        assert_eq!(result.rounds[1].eliminated, vec![3, 4]);
        assert_eq!(result.rounds[1].winner, None);
        // Round 3: 1 > 3, 2 > 3
        // No majority. 1, 2 are eliminated.
        assert_eq!(result.rounds[2].tallies, vec![(1, 3), (2, 3)]);
        assert_eq!(result.rounds[2].eliminated, vec![1, 2]);
        assert_eq!(result.rounds[2].winner, None);
    }
}
