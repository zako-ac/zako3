# Metrics Documentation

This document lists the metrics exported by the system.

## Emoji Matcher Metrics

| Metric Name | Type | Description | Labels |
| :--- | :--- | :--- | :--- |
| `emoji_register_requests_total` | Counter | Total number of emoji register requests | - |
| `emoji_match_requests_total` | Counter | Total number of emoji match requests | - |
| `emoji_match_hits_total` | Counter | Total number of emoji match hits | - |
| `emoji_hash_time_seconds` | Gauge | Time taken to hash an emoji | - |
| `emoji_db_query_duration_seconds` | Histogram | Latency of database queries | `operation` |
| `emoji_external_fetch_duration_seconds` | Histogram | Latency of external image fetches | `domain` |
| `emoji_nats_message_process_duration_seconds` | Histogram | Time spent processing a NATS request | `subject` |

