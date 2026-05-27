# Metrics Documentation

This document lists the metrics exported by the system.

## Emoji Matcher Metrics

| Metric Name | Type | Description | Labels |
| :--- | :--- | :--- | :--- |
| `emoji_scope_match_requests_total` | Counter | Scope-match requests received from HQ | - |
| `emoji_scope_match_drops_total` | Counter | Requests dropped because the task queue was full | - |
| `emoji_scope_match_hits_total` | Counter | Requests that resulted in a new mapping being written | - |
| `emoji_hash_time_seconds` | Gauge | Time taken to hash an emoji image | - |
