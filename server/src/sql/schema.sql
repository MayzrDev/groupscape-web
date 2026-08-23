CREATE SCHEMA IF NOT EXISTS groupscape;

CREATE TABLE IF NOT EXISTS groupscape.groups(
       group_id BIGSERIAL UNIQUE,
       group_name TEXT NOT NULL,
       group_token_hash CHAR(64) NOT NULL,
       PRIMARY KEY (group_name, group_token_hash)
);

-- NOTE: this file is not executed at startup - every table beyond `groups` above (including
-- this one) is actually created by the migration steps in `db.rs`'s `update_schema` (see
-- "create_item_bonuses_table"). Kept here for documentation/reference parity with that migration.
CREATE TABLE IF NOT EXISTS groupscape.item_bonuses (
       item_id INT PRIMARY KEY,
       attack_stab INT NOT NULL,
       attack_slash INT NOT NULL,
       attack_crush INT NOT NULL,
       attack_magic INT NOT NULL,
       attack_ranged INT NOT NULL,
       defence_stab INT NOT NULL,
       defence_slash INT NOT NULL,
       defence_crush INT NOT NULL,
       defence_magic INT NOT NULL,
       defence_ranged INT NOT NULL,
       melee_strength INT NOT NULL,
       ranged_strength INT NOT NULL,
       magic_damage INT NOT NULL,
       prayer INT NOT NULL,
       attack_speed INT,
       fetched_at TIMESTAMPTZ NOT NULL
);
