ALTER TABLE materials
    ADD COLUMN spool_weight_grams REAL NOT NULL DEFAULT 1000.0;

CREATE INDEX idx_orders_material_id ON orders(material_id);
