# Audio Engine Metrics

This document describes the Prometheus metrics exported by the Audio Engine.

## Global Metrics

| Metric Name | Type | Description |
|-------------|------|-------------|
| `audio_session_active_total` | Gauge | Number of active guild audio sessions. |

## Audio Pipeline Metrics

| Metric Name | Type | Description |
|-------------|------|-------------|
| `audio_mixer_active_sources` | Gauge | Current number of audio sources being mixed across all sessions. |
| `audio_mixer_processing_duration_seconds` | Histogram | Time taken for a single mixer loop iteration (target < 20ms). |
| `audio_mixer_underruns_total` | Counter | Total number of mixer underruns (starvation). |
| `audio_mixer_buffer_depth_samples` | Gauge | Current available samples in the mixer buffer. |
| `audio_decoder_stalls_total` | Counter | Total number of decoder stalls (buffer full). |
| `audio_stream_underruns_total` | Counter | Total number of output stream underruns. |
| `audio_decode_errors_total` | Counter | Total number of audio decoding errors. |

### Labels for `audio_decode_errors_total`
- `error_type`: `io`, `codec`, `format_probe`, `no_track`, `codec_init`, `sender_dropped`, `startup_timeout`.

## Track Lifecycle Metrics

| Metric Name | Type | Description | Labeled By |
|-------------|------|-------------|------------|
| `audio_track_lifecycle_total` | Counter | Track lifecycle events. | `event`, `queue_name` |
| `audio_preload_total` | Counter | Audio preload attempts and outcomes. | `result` |

### Labels
- `event`: `queued`, `start`, `end`, `stop`, `skip`.
- `queue_name`: `music`, `tts`, `other`.
- `result`: `hit`, `miss`, `error`.

## Integration Metrics (TapHub)

| Metric Name | Type | Description |
|-------------|------|-------------|
| `taphub_request_duration_seconds` | Histogram | Latency of TapHub API requests. |
| `taphub_errors_total` | Counter | Total number of TapHub request errors. |

### Labels for `taphub_errors_total`
- `endpoint`: `request_audio`, `preload_audio`, `request_audio_meta`.
