CREATE SCHEMA IF NOT EXISTS groupscape;

CREATE TABLE IF NOT EXISTS groupscape.groups(
       group_id BIGSERIAL UNIQUE,
       group_name TEXT NOT NULL,
       group_token_hash CHAR(64) NOT NULL,
       PRIMARY KEY (group_name, group_token_hash)
);
