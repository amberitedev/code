-- Cores report their state on start and on change, never on a timer. Reachability tests are
-- limited in the Worker's memory instead of a counter written on every test.

ALTER TABLE cores DROP COLUMN checks;
ALTER TABLE cores DROP COLUMN checks_window;
