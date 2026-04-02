# Zakofish

Zakofish is an audio routing and transmission protocol utilizing `protofish2` to connect audio processing Taps to a central Hub.

## Examples

To try out the connection flow locally, you can run the provided simple examples. First, start the Hub, and then start the Tap.

### 1. Run the Hub

The Hub acts as a server that listens for incoming connections from Taps:

```bash
cargo run --example hub
```

### 2. Run the Tap

In a new terminal window, run the Tap to connect to the listening Hub:

```bash
cargo run --example tap
```

Once connected, the Hub will report the incoming connection and log the Tap ID.
