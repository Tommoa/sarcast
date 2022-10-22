// @generated automatically by Diesel CLI.

diesel::table! {
    episodes (title, podcast_id) {
        title -> Text,
        uri -> Nullable<Text>,
        local_uri -> Nullable<Text>,
        description -> Nullable<Text>,
        epoch -> Integer,
        length -> Nullable<Integer>,
        duration -> Nullable<Integer>,
        guid -> Nullable<Text>,
        played -> Nullable<Integer>,
        play_position -> Integer,
        podcast_id -> Integer,
    }
}

diesel::table! {
    podcasts (id) {
        id -> Integer,
        title -> Text,
        link -> Text,
        description -> Text,
        image_uri -> Nullable<Text>,
        image_cached -> Timestamp,
        source_id -> Integer,
    }
}

diesel::table! {
    source (id) {
        id -> Integer,
        uri -> Text,
        last_modified -> Nullable<Text>,
        http_etag -> Nullable<Text>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    episodes,
    podcasts,
    source,
);
