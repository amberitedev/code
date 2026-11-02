-- The table of share history entries (`sharing_client::history`), the same in the App and Core.
-- Each side applies it after its own migrations. An entry's files are retained in the side's
-- content store under owner kind 'share_history'.

CREATE TABLE IF NOT EXISTS share_history (
	share_id TEXT NOT NULL,
	version INTEGER NOT NULL,
	-- JSON of `sharing_client::history::HistoryEntry`.
	entry TEXT NOT NULL,

	PRIMARY KEY (share_id, version)
);
