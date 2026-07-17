CREATE TABLE amberite_product_session (
    id INTEGER NOT NULL CHECK (id = 0),
    version INTEGER NOT NULL,
    encrypted_bundle BLOB NULL,
    nonce BLOB NULL,
    remembered_identity TEXT NULL,
    signed_out INTEGER NOT NULL DEFAULT TRUE CHECK (signed_out IN (0, 1)),
    updated_at INTEGER NOT NULL,

    PRIMARY KEY (id)
);
