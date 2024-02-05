// @generated automatically by Diesel CLI.

diesel::table! {
    tokens (id) {
        id -> Integer,
        token_name -> Text,
        token -> Text,
        token_status -> Text,
        project_id -> Text,
        project_status -> Text,
        bk -> Text,
        user_id -> Text,
        token_created_at -> Text,
    }
}
