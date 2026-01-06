import Fastify from "fastify";
import { Client, GatewayIntentBits } from "discord.js";

const fastify = Fastify({
  logger: true,
});

const client = new Client({
  intents: [GatewayIntentBits.Guilds],
});

fastify.get("/", async (_request, _reply) => {
  return { hello: "world" };
});

const start = async () => {
  try {
    // Start Fastify
    await fastify.listen({ port: 3000 });
    console.log("Fastify server is running");

    // Start Discord Bot (Token needed in environment variables)
    if (process.env.DISCORD_TOKEN) {
      await client.login(process.env.DISCORD_TOKEN);
      console.log("Discord bot logged in");
    } else {
      console.warn("No DISCORD_TOKEN found, bot not started");
    }
  } catch (err) {
    fastify.log.error(err);
    process.exit(1);
  }
};

start();
