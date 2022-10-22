-- Your SQL goes here
CREATE TABLE `source` (
    `id` INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT UNIQUE,
    `uri` TEXT NOT NULL UNIQUE,
    `last_modified` TEXT,
    `http_etag` TEXT
);

CREATE TABLE `episodes` (
    `title` TEXT NOT NULL,
    `uri` TEXT,
    `local_uri` TEXT,
    `description` TEXT,
    `epoch` INTEGER NOT NULL DEFAULT 0,
    `length` INTEGER,
    `duration` INTEGER,
    `guid` TEXT,
    `played` INTEGER,
    `play_position` INTEGER NOT NULL DEFAULT 0,
    `podcast_id` INTEGER NOT NULL,
    PRIMARY KEY (title, podcast_id)
);

CREATE TABLE `podcasts` (
    `id` INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT UNIQUE,
    `title` TEXT NOT NULL,
    `link` TEXT NOT NULL,
    `description` TEXT NOT NULL,
    `image_uri` TEXT,
    `image_cached` DATETIME NOT NULL,
    `source_id` INTEGER NOT NULL
);
