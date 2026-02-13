import type { FastifyInstance, FastifyPluginOptions } from "fastify";
import { healthRoutes, type HealthDependencies } from "./health.js";
import { settingsRoutes, type SettingsRouteDependencies } from "./settings.js";
import { userRoutes, type UserRouteDependencies } from "../api/user.routes.js";
import { tapRoutes, type TapRouteDependencies } from "../api/tap.routes.js";

export interface RouteDependencies
    extends
        HealthDependencies,
        SettingsRouteDependencies,
        UserRouteDependencies,
        TapRouteDependencies {}

export function registerRoutes(deps: RouteDependencies) {
    return async function (
        fastify: FastifyInstance,
        _opts: FastifyPluginOptions
    ) {
        await fastify.register(healthRoutes(deps));

        await fastify.register(
            async (apiInstance) => {
                await apiInstance.register(settingsRoutes(deps));
                await apiInstance.register(userRoutes(deps));
                await apiInstance.register(tapRoutes(deps));
            },
            { prefix: "/api/v1" }
        );
    };
}

export { healthRoutes, type HealthDependencies } from "./health.js";
export { settingsRoutes, type SettingsRouteDependencies } from "./settings.js";
