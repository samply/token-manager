// @generated automatically by Diesel CLI.

diesel::table! {
    tokens (id) {
        id -> Integer,
        token -> Text,
        project_id -> Text,
        bk -> Text,
        status -> Text,
        user_id -> Text,
        created_at -> Text,
    }
}
