# Zako3 Architecture

Zako3 has more sophisticated architecture than Zako2 infrastructure. 

![arch](/docs/assets/arch1.png)

Zako3 architecture is consisted of these modules.

- **HQ** *[Headquarter](services/hq.md)* HQ is a stateless service that runs basic backend operations like user account management, authentication. It also operates state machine of the entire Zako3 queue/track infrastructure.
- **AE** *[Audio Engine](services/ae.md)* AE is a stateful service that takes responsibility for specific audio session and performs various audio operations like mixing/queueing.
- [Removed by ADR](/docs/architecture/decisions/0002-merge-taphub-into-audio-engine.md) ~~**TH** *[TapHub](services/th.md)* TH is a stateful service that accepts [Protofish](https://github.com/zako-ac/protofish) connection from users and provides realtime audio source.~~
- **Bot** *[Bot](services/bot.md)* Bot is a stateless service that acts as a control surface of HQ in Discord.
- **BAE** *[Bot Audio Engine](services/bae.md)* BAE is a stateful service that bridges live audio data in AE to Discord Voice API.
- **Audit** *[Audit](services/audit.md)* Audit is a stateless service that receives metrics, logs, events from all services and provides observability. It also monitors for suspicious activity.
