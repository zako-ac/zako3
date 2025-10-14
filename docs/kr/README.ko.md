![Zako 3](https://capsule-render.vercel.app/api?type=waving&height=300&color=gradient&text=Zako%203)

# 3세대 Zako 시스템 구현
[![CI](https://github.com/zako-ac/zako3/actions/workflows/ci.yml/badge.svg)](https://github.com/zako-ac/zako3/actions/workflows/ci.yml)

---

[**English**](README.md) | [**한국어**](docs/kr/README.ko.md)

---

## Zako란 무엇인가요?
**Zako**는 기본적으로 디스코드 음악봇과 TTS 봇의 결합체 입니다. 이는 디스코드 음성 채널에 다양한 오디오 트랙을 제공합니다.

## 동기 부여
디스코드 문화에는 자신의 목소리를 드러내지 않는 사람들(즉, 마이크 없는 사용자)이 많습니다. 이들은 대게 마이크를 끄고 채팅을 통해서만 듣고 말합니다. 그러나 대부분의 사람들은 다른 일을 하고 있을 때 채팅 채널을 항상 볼 수는 없습니다. 그래서 개발자들은 TTS 기술을 사용하여 채팅 채널을 읽어주는 봇을 개발했습니다. 봇들은 매우 훌륭하고 잘 만들어졌지만, 저는 이러한 봇들에서 몇 가지 문제점과 더 나은 접근 방식을 발견했습니다.
### 문제점들
> 아래는 많은 디스코드 TTS 봇들이 겪는 문제점 입니다.
- **막힘 현상** 현재 메시지를 다 읽기 전까지는 다른 메시지를 읽지 않습니다.
- **맞춤형 기능 부족** 사용자가 소수의 TTS 음성만 사용할 수 있도록 허용합니다. 사용자는 그들의 음성을 커스터마이징할 수 없습니다.

## 기여자
- MincoMK (Discord: `@minco_rl`) - Zako
- Kanowara Sinoka (Discord: `@u_sinokadev`) - Devised a lot of features. The one made Zako3 exist.
- ridanit_ruma (Discord: `@ruma0607`) - 옮김
