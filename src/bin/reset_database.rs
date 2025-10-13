#[cfg(feature = "ssr")]
use hp_halloween_25::{establish_connection, reset_database};

#[cfg(feature = "ssr")]
fn main() {
    let mut conn = establish_connection();
    reset_database(&mut conn).expect("Failed to reset database");
    println!("Database has been reset.");
}

#[cfg(not(feature = "ssr"))]
fn main() {
    println!("This binary requires the 'ssr' feature to be enabled.");
}
