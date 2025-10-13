// @generated automatically by Diesel CLI, then modified by hand to remove Nullable almost
// everywhere.

diesel::table! {
    admin_sessions (id) {
        id -> Integer,
        token -> Text,
        create_at -> Timestamp,
        expires_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    guests (id) {
        id -> Integer,
        name -> Text,
        house_id -> Nullable<Integer>,
        personal_score -> Integer,
        is_active -> Integer,
        registered_at -> Nullable<Timestamp>,
        character -> Nullable<Text>,
    }
}

diesel::table! {
    houses (id) {
        id -> Integer,
        name -> Text,
        score -> Integer,
    }
}

diesel::table! {
    point_awards (id) {
        id -> Integer,
        guest_id -> Nullable<Integer>,
        house_id -> Nullable<Integer>,
        amount -> Integer,
        reason -> Text,
        awarded_at -> Timestamp,
    }
}

diesel::table! {
    sessions (id) {
        id -> Integer,
        guest_id -> Integer,
        token -> Text,
        created_at -> Timestamp,
        expires_at -> Timestamp,
    }
}

diesel::joinable!(guests -> houses (house_id));
diesel::joinable!(point_awards -> guests (guest_id));
diesel::joinable!(point_awards -> houses (house_id));
diesel::joinable!(sessions -> guests (guest_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_sessions,
    guests,
    houses,
    point_awards,
    sessions,
);
