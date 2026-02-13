CREATE TABLE "settings_entries" (
	"id" serial PRIMARY KEY NOT NULL,
	"key_identifier" varchar(255) NOT NULL,
	"scope_kind" varchar(20) NOT NULL,
	"scope_type" varchar(30) NOT NULL,
	"guild_id" varchar(30),
	"user_id" varchar(30),
	"value" jsonb NOT NULL,
	"is_important" boolean DEFAULT false NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "unique_key_scope" UNIQUE("key_identifier","scope_kind","scope_type","guild_id","user_id")
);
--> statement-breakpoint
CREATE INDEX "idx_settings_key" ON "settings_entries" USING btree ("key_identifier");--> statement-breakpoint
CREATE INDEX "idx_settings_scope_kind" ON "settings_entries" USING btree ("scope_kind");--> statement-breakpoint
CREATE INDEX "idx_settings_guild" ON "settings_entries" USING btree ("guild_id");--> statement-breakpoint
CREATE INDEX "idx_settings_user" ON "settings_entries" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "idx_settings_guild_user" ON "settings_entries" USING btree ("guild_id","user_id");