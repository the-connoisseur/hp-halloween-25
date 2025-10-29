// @generated automatically by Diesel CLI.

diesel::table! {
    admin_sessions (id) {
        id -> Integer,
        token -> Text,
        created_at -> Timestamp,
        expires_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    crossword_states (id) {
        id -> Integer,
        guest_id -> Integer,
        state -> Text,
        updated_at -> Timestamp,
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
    house_crossword_completions (id) {
        id -> Integer,
        house_id -> Integer,
        word_index -> Integer,
        completed_at -> Timestamp,
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

diesel::table! {
    votes (id) {
        id -> Integer,
        voter_id -> Integer,
        first_choice_id -> Integer,
        second_choice_id -> Integer,
        third_choice_id -> Integer,
        submitted_at -> Timestamp,
    }
}

diesel::table! {
    voting_status (id) {
        id -> Integer,
        is_open -> Integer,
        opened_at -> Nullable<Timestamp>,
        closed_at -> Nullable<Timestamp>,
    }
}

diesel::joinable!(crossword_states -> guests (guest_id));
diesel::joinable!(guests -> houses (house_id));
diesel::joinable!(house_crossword_completions -> houses (house_id));
diesel::joinable!(point_awards -> guests (guest_id));
diesel::joinable!(point_awards -> houses (house_id));
diesel::joinable!(sessions -> guests (guest_id));
diesel::joinable!(votes -> guests (voter_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_sessions,
    crossword_states,
    guests,
    house_crossword_completions,
    houses,
    point_awards,
    sessions,
    votes,
    voting_status,
);
