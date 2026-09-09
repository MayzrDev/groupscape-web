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

-- Also migration-created (see "add_groups_activity_reactions_enabled_column" in db.rs) - the
-- group-wide likes/comments toggle for the activity feed, default on.
ALTER TABLE groupscape.groups
ADD COLUMN IF NOT EXISTS activity_reactions_enabled BOOLEAN NOT NULL DEFAULT true;

-- Also migration-created (see "create_activity_event_reactions_table" in db.rs). One reaction per
-- (event, account); `groupscape.activity_events` is created by "create_sessions_and_activity_events_tables".
CREATE TABLE IF NOT EXISTS groupscape.activity_event_reactions (
       event_id BIGINT NOT NULL REFERENCES groupscape.activity_events(event_id) ON DELETE CASCADE,
       account_id BIGINT NOT NULL REFERENCES groupscape.accounts(id) ON DELETE CASCADE,
       reaction TEXT NOT NULL CHECK (reaction IN ('like', 'gg', 'lol', 'rare', 'f')),
       created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
       PRIMARY KEY (event_id, account_id)
);

-- Also migration-created (see "create_activity_event_comments_table" in db.rs). `member_name` is
-- a point-in-time snapshot of the commenting account's resolved member name, not a live FK.
CREATE TABLE IF NOT EXISTS groupscape.activity_event_comments (
       comment_id BIGSERIAL PRIMARY KEY,
       event_id BIGINT NOT NULL REFERENCES groupscape.activity_events(event_id) ON DELETE CASCADE,
       account_id BIGINT NOT NULL REFERENCES groupscape.accounts(id) ON DELETE CASCADE,
       member_name CITEXT NOT NULL,
       comment_text TEXT NOT NULL,
       created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
