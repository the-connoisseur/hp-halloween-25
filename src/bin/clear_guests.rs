#[cfg(feature = "ssr")]
use hp_halloween_25::{clear_all_guests, establish_connection};

#[cfg(feature = "ssr")]
fn main() {
    let mut conn = establish_connection();
    clear_all_guests(&mut conn).expect("Failed to clear guests");
    println!("All guests, sessions, and guest-specific point awards cleared.");
}

#[cfg(not(feature = "ssr"))]
fn main() {
    println!("This binary requires the 'ssr' feature to be enabled.");
}
