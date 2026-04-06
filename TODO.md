- [x] TapHub - AE comms

refer hq todo

# Do Now
- [x] Implement multiple AE

- [ ] Implement settings `~/projects/zako3/docs/en/settings.md`
- [ ] Implement text preprocessing
- [ ] implement cache: max cache size, score-based cache eviction worker
- [ ] add reconnection logic
- [ ] processing time measurement

# Other
- [ ] otel and prometheus and logging(promtail)
- [x] AE Track proto handling (todo!)
- [x] migrate AE-TH comms to jsonrpsee
- [ ] Make AE work without TH (atleast no crash)
- [ ] Migrate vendor to fork
- [x] change IDs from Uuid to String
- [x] check admin verification api
- [x] Make background metrics saver
- [x] Make HQ RPC for zakoctl, and use it to make someone admin
- [ ] merge migrations
- [ ] /api/v1/taps/299197284933963776/report endpoint
- [x] verify tap in web
- [ ] implement ban
- [ ] AE Hang on no join
- [ ] AE re-join on boot
- [ ] AE init rmq AFTER init discord
- [ ] Add timeout to AE client
- [ ] Add track finish to AE queue

- [x] verify_permission in TH handler.rs -> Add method in HQ RPC and use it to get user info.

## Web
- [x] When browsing tap settings or tap stats, make "stats", "settings", "create api key" in sub-sidebar. Separate API key creation UI to component from settings if needed.
- [x] Make *Create Tap* button in top bar
- [x] In audit log, event is not displayed and user is displayed as ID -> refactor user badge in tap info as a component then use it
- [x] Increase border contrast
- [x] My Taps card in Dashboard: change hover color from primary to something else, maybe a light gray? (or maybe just make it more subtle)
- [ ] unread-count
- [ ] Improve chart
- [ ] Add Cache hit rate in cache hits card
- [ ] Make all cards in settings as component
- [ ] Add "load failed" 
- [ ] Dashboard status (my taps, total uses, active users, uptime)
- [x] Add admin UI/API to change tap settings
- [x] Make admin tap details UI use StatsCard component instead of manual impl
