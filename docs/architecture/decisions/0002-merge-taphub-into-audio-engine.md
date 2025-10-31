# 2. Merge TapHub into Audio Engine

Date: 2025-10-31

## Status

Accepted

## Context

In a previous version Zako2, role of Audio Engine and TapHub were solid. Audio Engine directly joined Discord VC, and TapHub handled communication and management of Taps. On the contrary, Zako3 has a dedicaded Discord VC handling layer, namely, Bot Audio Angine. The only reason we adopted TH is simply that we've continued the old practice. Yes it is good, but only in Zako2 not Zako3.

## Decision

Merge TapHub into AE.

## Consequences

- Significantly frees up the development time.
- Further network-level distribution is possible. Audio Engine can give its own IP to Tap, and avoid Zako3 proxying traffic overload.
