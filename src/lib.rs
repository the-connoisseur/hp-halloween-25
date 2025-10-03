pub mod app;
pub mod model;
#[cfg(feature = "ssr")]
pub mod schema;

#[cfg(feature = "ssr")]
use diesel::connection::SimpleConnection;
#[cfg(feature = "ssr")]
use diesel::prelude::*;
#[cfg(feature = "ssr")]
use diesel::SqliteConnection;
#[cfg(feature = "ssr")]
use dotenvy::dotenv;
#[cfg(feature = "ssr")]
use std::env;
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
use crate::model::{Guest, House, NewGuest, NewPointAward, NewSession, PointAward};
#[cfg(feature = "ssr")]
use crate::schema::{guests, houses, point_awards, sessions};

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
        "PRAGMA journal_mode = WAL; \
        PRAGMA synchronous = NORMAL; \
        PRAGMA busy_timeout = 10000;",
    )
    .expect("Failed to set SQLite PRAGMAs");

    conn
}

/// Registers a new guest, assigns them to a house, and generates a session token.
/// Returns the guest and token string.
#[cfg(feature = "ssr")]
pub fn register_guest(
    conn: &mut SqliteConnection,
    name: &str,
    house_id: i32,
) -> Result<(Guest, String), diesel::result::Error> {
    conn.transaction(|conn| {
        // Validate house exists.
        let house_exists: i64 = houses::table
            .filter(houses::id.eq(house_id))
            .count()
            .get_result(conn)?;
        if house_exists == 0 {
            return Err(diesel::result::Error::NotFound);
        }

        // Insert guest and get the new ID..
        let new_guest = NewGuest { name, house_id };
        let inserted_id: i32 = diesel::insert_into(guests::table)
            .values(&new_guest)
            .returning(guests::id)
            .get_result(conn)?;

        // Fetch the full inserted guest.
        let guest: Guest = guests::table
            .find(inserted_id)
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

/// Reregisters a guest: Reactivates them, optionally changes house, deletes old session (if any),
/// and generates a new token.
/// Returns updated guest and new token if an entry for this guest already exists, or NotFound
/// error otherwise.
#[cfg(feature = "ssr")]
pub fn reregister_guest(
    conn: &mut SqliteConnection,
    guest_id: i32,
    new_house_id: Option<i32>,
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
                .set(guests::house_id.eq(house_id))
                .execute(conn)?;
            guest.house_id = house_id;
        }

        // Reactivate.
        diesel::update(guests::table.filter(guests::id.eq(guest_id)))
            .set(guests::is_active.eq(1i32))
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
        // First fetch the guest and their house from the database.
        let (guest, house): (Guest, House) = guests::table
            .filter(guests::id.eq(guest_id))
            .inner_join(houses::table.on(guests::house_id.eq(houses::id)))
            .select((Guest::as_select(), House::as_select()))
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
        };
        diesel::insert_into(point_awards::table)
            .values(&new_award)
            .get_result(conn)
    })
}

/// Fetches all houses.
#[cfg(feature = "ssr")]
pub fn get_all_houses(conn: &mut SqliteConnection) -> Result<Vec<House>, diesel::result::Error> {
    houses::table
        .order(houses::name)
        .select(House::as_select())
        .load(conn)
}

/// Fetches a guest's details, including their house.
#[cfg(feature = "ssr")]
pub fn get_guest_details(
    conn: &mut SqliteConnection,
    guest_id: i32,
) -> Result<(Guest, House), diesel::result::Error> {
    guests::table
        .filter(guests::id.eq(guest_id))
        .inner_join(houses::table.on(guests::house_id.eq(houses::id)))
        .filter(guests::is_active.eq(1i32))
        .select((Guest::as_select(), House::as_select()))
        .first(conn)
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use crate::schema::houses::dsl::*;

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
            // Register a guest and verify the entry.
            let (guest, token) =
                register_guest(conn, "Test Guest", 1).expect("Failed to register guest");
            assert_eq!(guest.name, "Test Guest");
            assert_eq!(guest.house_id, 1);
            assert_eq!(guest.is_active, 1);
            assert!(!token.is_empty());
            assert!(Uuid::parse_str(&token).is_ok());

            // Verify the session exists.
            let session_count: i64 = sessions::table
                .filter(sessions::token.eq(&token))
                .count()
                .get_result(conn)
                .expect("Session count failed");
            assert_eq!(session_count, 1);

            Ok(())
        });
    }

    #[test]
    fn test_get_guest_by_token() {
        run_test_in_transaction(|conn| {
            // Register a guest.
            let (guest, token) =
                register_guest(conn, "Token Guest", 2).expect("Failed to register guest");

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
            // Register a guest.
            let (guest, _) =
                register_guest(conn, "Unregister Guest", 3).expect("Failed to register guest");

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
            // Register, then unregister a guest.
            let (guest, _) =
                register_guest(conn, "Reregister Guest", 4).expect("Failed to register guest");
            unregister_guest(conn, guest.id).expect("Failed to unregister guest");

            // Reregister with new house.
            let (reregistered, new_token) =
                reregister_guest(conn, guest.id, Some(2)).expect("Failed to reregister guest");
            assert_eq!(reregistered.id, guest.id);
            assert_eq!(reregistered.house_id, 2);
            assert_eq!(reregistered.is_active, 1i32);
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
                reregister_guest(conn, guest.id, None).expect("Failed to reregister guest");
            assert_eq!(no_change.house_id, 2);
            assert_ne!(no_change_token, new_token);

            // Reregister a guest that doesn't exist, verify that an error is returned.
            assert!(reregister_guest(conn, 42, None).is_err());

            // Reregister a guest with a house that doesn't exist, verify that an error is returned.
            assert!(reregister_guest(conn, guest.id, Some(69)).is_err());

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
            // Register a guest with Gryffindor.
            let (guest, _) = register_guest(conn, "Hagrid", 1)?;
            let guest_id = guest.id;

            // Read the guest details and verify that they are correct.
            let (guest, house) = get_guest_details(conn, guest_id)?;
            assert_eq!(guest.id, guest_id);
            assert_eq!(guest.name, "Hagrid");
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
            // Register 3 guests - 2 in Gryffindor and 1 in Slytherin.
            let (gryffindor_guest_1, _) = register_guest(conn, "Gryffindor Guest 1", 1)?;
            let (gryffindor_guest_2, _) = register_guest(conn, "Gryffindor Guest 2", 1)?;
            let (slytherin_guest, _) = register_guest(conn, "Slytherin Guest", 4)?;

            // Award points to first Gryffindor guest, and verify the contents of the returned value.
            let award = award_points_to_guest(conn, gryffindor_guest_1.id, 10, "Game win")?;
            assert_eq!(award.amount, 10);
            assert_eq!(award.reason, "Game win");
            assert_eq!(award.guest_id, Some(gryffindor_guest_1.id));

            // Read the guest details and verify the individual and house points.
            let (gryffindor_guest_1, gryffindor) = get_guest_details(conn, gryffindor_guest_1.id)?;
            assert_eq!(gryffindor_guest_1.personal_score, 10);
            assert_eq!(gryffindor.score, 10);

            // Deduct points from the same guest. Read the guest details and verify the individual
            // and house points.
            award_points_to_guest(conn, gryffindor_guest_1.id, -5, "Penalty")?;
            let (gryffindor_guest_1, gryffindor) = get_guest_details(conn, gryffindor_guest_1.id)?;
            assert_eq!(gryffindor_guest_1.personal_score, 5);
            assert_eq!(gryffindor.score, 5);

            // Award points to second Gryffindor guest. Read the guest details and verify the
            // individual and house points.
            award_points_to_guest(conn, gryffindor_guest_2.id, 20, "Game win")?;
            let (gryffindor_guest_2, gryffindor) = get_guest_details(conn, gryffindor_guest_2.id)?;
            assert_eq!(gryffindor_guest_2.personal_score, 20);
            assert_eq!(gryffindor.score, 25);

            // Award points to Slytherin guest. Read the guest details and verify the individual
            // and house points.
            award_points_to_guest(conn, slytherin_guest.id, 15, "Game win")?;
            let (slytherin_guest, slytherin) = get_guest_details(conn, slytherin_guest.id)?;
            assert_eq!(slytherin_guest.personal_score, 15);
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
}
