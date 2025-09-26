pub mod app;
pub mod model;
pub mod schema;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::SqliteConnection;
use dotenvy::dotenv;
use std::env;
use uuid::Uuid;

use crate::model::{Guest, NewGuest, NewSession};
use crate::schema::{guests, houses, sessions};

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

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

#[cfg(test)]
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
}
