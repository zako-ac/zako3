# Zako3 Architecture

Zako3 is community-driven audio bot that lives in Discord.

## The Problem
Many Discord servers seek to enhance their member experience with music and audio bots. However, existing solutions often lack a lot of features.

- **Fragmentation**: The big problem. Majority of servers have a tons of music bots, each with different features and capabilities. Members have to learn how to use each of them, often consuming time of *tech personnel*.
- **Unsophisticated Features**: This is also partially derived from fragmentation. TTS/Music bot is more complicated than it looks. People expect the bot to automatically know to read emojis, queue songs, and so on, without need of their explanations. It leads for developers to continuously use and research the user experience.

## The Solution
The solution is simple. Unification, and community-driven customization. Zako3 aims to be the one-stop solution for all audio needs in Discord servers. Zako3 allows users to build their own audio source, including TTS voices, music sources; by themselves. Therefore, users can select their preferred audio sources from tons of community-made options, but retaining a single intuitive interface.

## Taps
Zako introduces a concept of **Taps**. Taps are units of audio sources that users can *select* to use. Each Tap can represent a TTS voice, a music source, or any other audio source. Users can browse and use Taps created by the community, or create their own Taps to share with others.

### Technical Side of Taps
Users can create a Tap using Zako3's Tap SDK. The SDK provides a simple server software that users can run to host their Tap. The Tap server communicates with Zako3 bot, which plays audio from the Tap in Discord voice channels.
