CREATE TABLE IF NOT EXISTS tap_metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tap_id UUID NOT NULL REFERENCES taps(id) ON DELETE CASCADE,
    metric_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS tap_metrics_tap_id_idx ON tap_metrics(tap_id);
CREATE INDEX IF NOT EXISTS tap_metrics_created_at_idx ON tap_metrics(created_at);
