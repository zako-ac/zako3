- [x] TapHub - AE comms

refer hq todo

# Do Now
- [x] Fix graph
- [x] use SSE for live update in queue and stats

- [ ] add AE controller to zakoctl
- [ ] AE controller -> boot

- [ ] re-join logic in HQ/AE
- [ ] Add translations for errors in bot
- [ ] Fix mention matching
- [ ] 첨부 파일 N개와 함께
- [ ] 증발 not work in global settings

# Done
- [x] Implement multiple AE
- [x] Implement settings `~/projects/zako3/docs/en/settings.md`
- [x] add reconnection logic
- [x] Add redirect in OAuth2 URL, so user can login and then be redirected to the page they were trying to access before login instead of always being redirected to dashboard.
- [x] Add uptime and active users
- [x] refactor TH
- [x] implement cache: max cache size, score-based cache eviction worker
- [x] Implement text preprocessing
- [x] Make bot leave when VC is empty
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
- [x] Gauge metrics reset on TH boot
- [x] Add track finish to AE queue
- [x] Add timeout to AE client
- [x] Add base volume for tap
- [x] Make TH connection ID (online count) tracking better
- [x] AE re-join on boot
- [x] Pause feature

- [x] otel and prometheus and logging(promtail)
- [x] Plan proper logging with context
- [x] Refine errors
- [x] Make tap-sdk
- [x] tts commands TODO resolve
- [x] Remove dependencies of Zakofish
- [x] Make cache get removed when early stop
- [x] Async decoding performance issue in pf2 maybe?

# Other
- [ ] merge migrations
- [ ] /api/v1/taps/299197284933963776/report endpoint

- [ ] Cache management admin panel
- [ ] emoji matching service
- [ ] Emoji/Text mapping scope elevation request
- [ ] Standardize service folder structure to "boot"
- [ ] pre-settings: Admin can change UserSettings even user is not registered
- [ ] Show Duration: maybe Tap "capabilities"?

## Do before production
- [ ] Captcha
- [ ] implement ban
- [ ] Update Zako web

## Possible future features
- [ ] AR PArameters
- [ ] search API in tap

## Text Preprocessing
- [ ] ㅏ -> 아

## Web
- [x] When browsing tap settings or tap stats, make "stats", "settings", "create api key" in sub-sidebar. Separate API key creation UI to component from settings if needed.
- [x] Make *Create Tap* button in top bar
- [x] In audit log, event is not displayed and user is displayed as ID -> refactor user badge in tap info as a component then use it
- [x] Increase border contrast
- [x] My Taps card in Dashboard: change hover color from primary to something else, maybe a light gray? (or maybe just make it more subtle)
- [x] unread-count
- [x] Improve chart
- [x] Make all cards in settings as component
- [x] Dashboard status (my taps, total uses, active users, uptime)
- [x] Add admin UI/API to change tap settings
- [x] Make admin tap details UI use StatsCard component instead of manual impl
- [x] Localhost CA change

- [x] Test Korean and refine i18n
- [ ] Add "load failed" 
- [x] Remove empty queue
- [x] Add reload button and periodic reload in queue,
- [x] Add avatar and username to queue
- [ ] reverse sort

## Mobile
- [x] Tap card overflow
- [ ] Make sidebar bigger, close it when selecting something
- [ ] Add left-drag sidebar

## Discord
- [x] Separate embeds and add theme color
- [x] Don't read itself
- [x] Use plain text for everyday messages, for friendly UX.
