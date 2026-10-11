-- Reaching a Core from outside its home network.

-- SHA-256 of the certificate a Core serves its API with. The App trusts only this certificate for
-- that Core. Null for a Core that has not started since it learned to serve HTTPS.
ALTER TABLE core_pairing_codes ADD COLUMN cert_fingerprint TEXT;
ALTER TABLE cores ADD COLUMN cert_fingerprint TEXT;

-- Where the Core's API is on the internet, once the Core has proven it can be reached there.
-- `address` stays the address on its own network.
ALTER TABLE cores ADD COLUMN public_address TEXT;

-- Outside reachability tests this Core asked for since `checks_window`.
ALTER TABLE cores ADD COLUMN checks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE cores ADD COLUMN checks_window TEXT;
