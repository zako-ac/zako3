I'm making services/traffic-light.
Currently, services/audio-engine identifies session(session is guild ID + channel ID) by NATS subject. However, it's difficult to distinguish errors and properly manage multiple AEs only with NATS.
So it's what traffic-light is for. I'll call it TL.

# Features
## Routing
TL receives requests from Tarpc this requests should include idempotency key. The requests are defined in `crates/tl-protocol/src/lib.rs` (the only file in that crate). TL routes the requests to the corresponding AEs and gets the response.
Routing algorithm is stored in `services/traffic-light/core/src/router.rs`.
Also communicateion with audio-engine is not implemented yet. So leave it as a trait.

## Voice State Tracking
TL tracks voice states of audio-engine sessions. So if it receives external state change like getting kicked out of VC by admin, it can trigger the leave command.

## Misc
### Healthcheck
TL has a healthcheck endpoint. It can be used for monitoring and load balancers.
Simple `GET /health` endpoint that returns 200 OK if TL is running properly.

### Logging
Use `tracing` crate (just run `cargo add tracing` since it's in workspace) for logging. Logging should be structured and include relevant information for debugging and monitoring. For example, log incoming requests, routing decisions, and any errors that occur during processing. Also, actively make spans. Also, use `headers` field in AudioEngineCommandRequest to track cross-service plans.

### Tracing
Add `OTLP_ENDPOINT` and integration as other projects. Refer `zako3-telemetry`
