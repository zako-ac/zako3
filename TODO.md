- [x] TapHub - AE comms

refer hq todo

# Do Now
- [x] Implement multiple AE

- [ ] Implement text preprocessing
- [ ] implement cache: max cache size, score-based cache eviction worker
- [x] Implement settings `~/projects/zako3/docs/en/settings.md`
- [x] add reconnection logic
- [x] Add redirect in OAuth2 URL, so user can login and then be redirected to the page they were trying to access before login instead of always being redirected to dashboard.
- [x] Add uptime and active users


# Other
- [x] AE Track proto handling (todo!)
- [x] migrate AE-TH comms to jsonrpsee
- [x] Make AE work without TH (atleast no crash)
- [x] change IDs from Uuid to String
- [x] check admin verification api
- [x] Make background metrics saver
- [x] Make HQ RPC for zakoctl, and use it to make someone admin
- [x] verify tap in web
- [x] AE init rmq AFTER init discord
- [x] AE Hang on no join
- [x] Add timeout to audio request
- [x] verify_permission in TH handler.rs -> Add method in HQ RPC and use it to get user info.

- [ ] otel and prometheus and logging(promtail)
- [ ] merge migrations
- [ ] /api/v1/taps/299197284933963776/report endpoint
- [ ] implement ban

- [ ] AE re-join on boot
- [ ] Gauge metrics reset on TH boot
- [ ] Add track finish to AE queue
- [ ] Add timeout to AE client
- [ ] Add search API for tap (like youtube search)
- [ ] Make TH connection ID (online count) tracking better
- [ ] Make cache removation command

## Text Preprocessing
- [ ] ㅏ -> 아

## Web
- [x] When browsing tap settings or tap stats, make "stats", "settings", "create api key" in sub-sidebar. Separate API key creation UI to component from settings if needed.
- [x] Make *Create Tap* button in top bar
- [x] In audit log, event is not displayed and user is displayed as ID -> refactor user badge in tap info as a component then use it
- [x] Increase border contrast
- [x] My Taps card in Dashboard: change hover color from primary to something else, maybe a light gray? (or maybe just make it more subtle)
- [ ] unread-count
- [x] Improve chart
- [x] Make all cards in settings as component
- [ ] Add "load failed" 
- [x] Dashboard status (my taps, total uses, active users, uptime)
- [x] Add admin UI/API to change tap settings
- [x] Make admin tap details UI use StatsCard component instead of manual impl

## Discord
- [ ] Separate embeds and add theme color
- [ ] Don't read itself
- [ ] Use plain text for everyday messages, for friendly UX.
- [ ] Tap list command: max 10 minutes live update
