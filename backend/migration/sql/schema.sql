DO $$ BEGIN
    CREATE TYPE public.offer_status AS ENUM ('pending', 'accepted', 'rejected', 'timeout');
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE public.payment_method_enum AS ENUM ('credit_card', 'bank_transfer', 'cash_on_delivery', 'pickup');
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE public.ride_status AS ENUM ('searching', 'accepted', 'picked_up', 'completed', 'cancelled', 'no_driver');
EXCEPTION WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS public.cities (id bigint NOT NULL, country_id bigint NOT NULL, name character varying NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.cities_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.cities_id_seq OWNED BY public.cities.id;

CREATE TABLE IF NOT EXISTS public.comments (id bigint NOT NULL, user_id bigint NOT NULL, lang character varying NOT NULL, content_type character varying NOT NULL, content_id bigint NOT NULL, content text NOT NULL, star integer DEFAULT 5 NOT NULL, publish boolean DEFAULT true NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP, updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP, deleted_at timestamp with time zone, ip_address character varying(50));
CREATE SEQUENCE IF NOT EXISTS public.comments_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.comments_id_seq OWNED BY public.comments.id;

CREATE TABLE IF NOT EXISTS public.content_terms (id bigint NOT NULL, content_id bigint NOT NULL, term_id bigint NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP, content_type character varying(50) DEFAULT 'page'::character varying NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.content_terms_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.content_terms_id_seq OWNED BY public.content_terms.id;

CREATE SEQUENCE IF NOT EXISTS public.contents_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE TABLE IF NOT EXISTS public.contents (id bigint DEFAULT nextval('public.contents_id_seq'::regclass) NOT NULL, data jsonb, publish boolean DEFAULT false, created_at timestamp with time zone, updated_at timestamp with time zone, deleted_at timestamp with time zone, content_type text DEFAULT 'page'::text, parent_id bigint, order_id integer, user_id bigint, gcx boolean DEFAULT false NOT NULL);

CREATE TABLE IF NOT EXISTS public.countries (id bigint NOT NULL, name character varying NOT NULL, code character varying, phone_code character varying);
CREATE SEQUENCE IF NOT EXISTS public.countries_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.countries_id_seq OWNED BY public.countries.id;

CREATE TABLE IF NOT EXISTS public.districts (id bigint NOT NULL, city_id bigint NOT NULL, name character varying NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.districts_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.districts_id_seq OWNED BY public.districts.id;

CREATE TABLE IF NOT EXISTS public.drivers (id bigint NOT NULL, user_id bigint, full_name character varying NOT NULL, phone character varying NOT NULL, vehicle_plate character varying NOT NULL, vehicle_model character varying NOT NULL, rating double precision DEFAULT 5.0 NOT NULL, is_active boolean DEFAULT true NOT NULL, is_online boolean DEFAULT false NOT NULL, current_lat double precision, current_lon double precision, location_updated_at timestamp with time zone, created_at timestamp with time zone DEFAULT now() NOT NULL, updated_at timestamp with time zone DEFAULT now() NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.drivers_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.drivers_id_seq OWNED BY public.drivers.id;

CREATE TABLE IF NOT EXISTS public.form_submissions (id bigint NOT NULL, form_id bigint NOT NULL, data jsonb NOT NULL, ip character varying, user_id bigint, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP, updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP);
CREATE SEQUENCE IF NOT EXISTS public.form_submissions_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.form_submissions_id_seq OWNED BY public.form_submissions.id;

CREATE TABLE IF NOT EXISTS public.homepage (id bigint NOT NULL, data jsonb DEFAULT '[]'::jsonb NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP, updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP);
CREATE SEQUENCE IF NOT EXISTS public.homepage_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.homepage_id_seq OWNED BY public.homepage.id;

CREATE TABLE IF NOT EXISTS public.locations (id bigint NOT NULL, name text NOT NULL, address text DEFAULT ''::text NOT NULL, lat double precision NOT NULL, lon double precision NOT NULL, category character varying(64) DEFAULT 'other'::character varying NOT NULL, is_active boolean DEFAULT true NOT NULL, created_at timestamp with time zone DEFAULT now() NOT NULL, updated_at timestamp with time zone DEFAULT now() NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.locations_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.locations_id_seq OWNED BY public.locations.id;

CREATE TABLE IF NOT EXISTS public.mail_queue (id bigint NOT NULL, to_email character varying NOT NULL, to_name character varying, subject character varying NOT NULL, body text NOT NULL, variables json, language character varying DEFAULT 'tr'::character varying, status character varying DEFAULT 'pending'::character varying, attempts integer DEFAULT 0, max_attempts integer DEFAULT 3, error_message text, scheduled_at timestamp with time zone, sent_at timestamp with time zone, created_at timestamp with time zone, updated_at timestamp with time zone, template_name character varying);
CREATE SEQUENCE IF NOT EXISTS public.mail_queue_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.mail_queue_id_seq OWNED BY public.mail_queue.id;

CREATE TABLE IF NOT EXISTS public.media (id bigint NOT NULL, user_id integer NOT NULL, file_name character varying NOT NULL, media_type character varying NOT NULL, mime_type character varying NOT NULL, file_path character varying NOT NULL, file_size bigint NOT NULL, title character varying, description text, content_type character varying, content_id bigint, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL, updated_at timestamp with time zone);
CREATE SEQUENCE IF NOT EXISTS public.media_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.media_id_seq OWNED BY public.media.id;

CREATE TABLE IF NOT EXISTS public.password_resets (id bigint NOT NULL, user_id bigint NOT NULL, token character varying NOT NULL, email character varying NOT NULL, expires_at timestamp with time zone NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.password_resets_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.password_resets_id_seq OWNED BY public.password_resets.id;

CREATE TABLE IF NOT EXISTS public.permissions (id bigint NOT NULL, name character varying NOT NULL, description text, module character varying NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP);
CREATE SEQUENCE IF NOT EXISTS public.permissions_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.permissions_id_seq OWNED BY public.permissions.id;

CREATE TABLE IF NOT EXISTS public.ride_fare_configs (id bigint NOT NULL, city_code character varying(50) NOT NULL, city_name character varying(100) NOT NULL, opening_fee numeric(10,2) DEFAULT 15.00 NOT NULL, min_fare numeric(10,2) DEFAULT 25.00 NOT NULL, per_km_fee numeric(10,2) DEFAULT 8.00 NOT NULL, is_active boolean DEFAULT true NOT NULL, created_at timestamp with time zone DEFAULT now() NOT NULL, updated_at timestamp with time zone DEFAULT now() NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.ride_fare_configs_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.ride_fare_configs_id_seq OWNED BY public.ride_fare_configs.id;

CREATE TABLE IF NOT EXISTS public.ride_offers (id bigint NOT NULL, ride_id bigint NOT NULL, driver_id bigint NOT NULL, status public.offer_status DEFAULT 'pending'::public.offer_status NOT NULL, offer_order integer NOT NULL, offered_at timestamp with time zone DEFAULT now() NOT NULL, responded_at timestamp with time zone);
CREATE SEQUENCE IF NOT EXISTS public.ride_offers_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.ride_offers_id_seq OWNED BY public.ride_offers.id;

CREATE TABLE IF NOT EXISTS public.rides (id bigint NOT NULL, user_id bigint NOT NULL, driver_id bigint, status public.ride_status DEFAULT 'searching'::public.ride_status NOT NULL, pickup_lat double precision NOT NULL, pickup_lon double precision NOT NULL, pickup_address text NOT NULL, dropoff_lat double precision NOT NULL, dropoff_lon double precision NOT NULL, dropoff_address text NOT NULL, distance_km double precision, duration_sec integer, fare_amount numeric(10,2), requested_at timestamp with time zone DEFAULT now() NOT NULL, accepted_at timestamp with time zone, picked_up_at timestamp with time zone, completed_at timestamp with time zone, cancelled_at timestamp with time zone);
CREATE SEQUENCE IF NOT EXISTS public.rides_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.rides_id_seq OWNED BY public.rides.id;

CREATE TABLE IF NOT EXISTS public.role_permissions (role_id bigint NOT NULL, permission_id bigint NOT NULL);
CREATE TABLE IF NOT EXISTS public.roles (id bigint NOT NULL, name character varying NOT NULL, description text, is_system boolean DEFAULT false NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP, updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP);
CREATE SEQUENCE IF NOT EXISTS public.roles_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.roles_id_seq OWNED BY public.roles.id;

CREATE TABLE IF NOT EXISTS public.sessions (id character varying NOT NULL, user_id bigint, data json NOT NULL, expires_at timestamp with time zone NOT NULL, created_at timestamp with time zone, updated_at timestamp with time zone);
CREATE TABLE IF NOT EXISTS public.settings (id bigint NOT NULL, data jsonb, created_at timestamp with time zone, updated_at timestamp with time zone);
CREATE SEQUENCE IF NOT EXISTS public.settings_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.settings_id_seq OWNED BY public.settings.id;

CREATE TABLE IF NOT EXISTS public.terms (id bigint NOT NULL, vocabulary_id bigint NOT NULL, data json NOT NULL, parent_id bigint, publish boolean DEFAULT true NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL, updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL, order_id integer, lock boolean DEFAULT false NOT NULL, hide boolean DEFAULT false NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.terms_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.terms_id_seq OWNED BY public.terms.id;

CREATE TABLE IF NOT EXISTS public.timeline_events (id bigint NOT NULL, module_type character varying(50) NOT NULL, content_type character varying(50) NOT NULL, content_id bigint NOT NULL, event_type character varying(100) NOT NULL, title json NOT NULL, description json, icon character varying(50), color character varying(20) DEFAULT 'primary'::character varying, user_id bigint, admin_user_id bigint, metadata json, is_public boolean DEFAULT true NOT NULL, is_admin_only boolean DEFAULT false NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP, updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP);
CREATE SEQUENCE IF NOT EXISTS public.timeline_events_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.timeline_events_id_seq OWNED BY public.timeline_events.id;

CREATE TABLE IF NOT EXISTS public.user_permissions (user_id bigint NOT NULL, permission_id bigint NOT NULL, is_granted boolean DEFAULT true NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP);
CREATE TABLE IF NOT EXISTS public.user_roles (user_id bigint NOT NULL, role_id bigint NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP);

CREATE TABLE IF NOT EXISTS public.users (id bigint NOT NULL, username character varying NOT NULL, first_name character varying, last_name character varying, birth_date timestamp with time zone, email character varying NOT NULL, password character varying, created_at timestamp with time zone, updated_at timestamp with time zone, profile jsonb, google_id character varying, apple_id character varying, is_guest boolean DEFAULT false NOT NULL, guest_session_id character varying, phone_number character varying, phone_country_code character varying DEFAULT '+90'::character varying, ip character varying(254), ip_v6 character varying(254), user_type character varying DEFAULT 'B2C'::character varying NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.users_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.users_id_seq OWNED BY public.users.id;

CREATE TABLE IF NOT EXISTS public.vocabularies (id bigint NOT NULL, data json NOT NULL, vocabulary_type character varying DEFAULT 'category'::character varying NOT NULL, created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL, updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL, order_id integer, gcx boolean DEFAULT false NOT NULL, lock boolean DEFAULT false NOT NULL, hide boolean DEFAULT false NOT NULL);
CREATE SEQUENCE IF NOT EXISTS public.vocabularies_id_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
ALTER SEQUENCE public.vocabularies_id_seq OWNED BY public.vocabularies.id;

CREATE TABLE IF NOT EXISTS public.vocabulary_categories (vocabulary_id bigint NOT NULL, category_term_id bigint NOT NULL, created_at timestamp with time zone DEFAULT now());

DO $$ BEGIN ALTER TABLE ONLY public.cities ALTER COLUMN id SET DEFAULT nextval('public.cities_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.comments ALTER COLUMN id SET DEFAULT nextval('public.comments_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.content_terms ALTER COLUMN id SET DEFAULT nextval('public.content_terms_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.countries ALTER COLUMN id SET DEFAULT nextval('public.countries_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.districts ALTER COLUMN id SET DEFAULT nextval('public.districts_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.drivers ALTER COLUMN id SET DEFAULT nextval('public.drivers_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.form_submissions ALTER COLUMN id SET DEFAULT nextval('public.form_submissions_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.homepage ALTER COLUMN id SET DEFAULT nextval('public.homepage_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.locations ALTER COLUMN id SET DEFAULT nextval('public.locations_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.mail_queue ALTER COLUMN id SET DEFAULT nextval('public.mail_queue_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.media ALTER COLUMN id SET DEFAULT nextval('public.media_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.password_resets ALTER COLUMN id SET DEFAULT nextval('public.password_resets_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.permissions ALTER COLUMN id SET DEFAULT nextval('public.permissions_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.ride_fare_configs ALTER COLUMN id SET DEFAULT nextval('public.ride_fare_configs_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.ride_offers ALTER COLUMN id SET DEFAULT nextval('public.ride_offers_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.rides ALTER COLUMN id SET DEFAULT nextval('public.rides_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.roles ALTER COLUMN id SET DEFAULT nextval('public.roles_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.settings ALTER COLUMN id SET DEFAULT nextval('public.settings_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.terms ALTER COLUMN id SET DEFAULT nextval('public.terms_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.timeline_events ALTER COLUMN id SET DEFAULT nextval('public.timeline_events_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.users ALTER COLUMN id SET DEFAULT nextval('public.users_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;
DO $$ BEGIN ALTER TABLE ONLY public.vocabularies ALTER COLUMN id SET DEFAULT nextval('public.vocabularies_id_seq'::regclass); EXCEPTION WHEN duplicate_object THEN null; END $$;

DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'cities_pkey') THEN ALTER TABLE ONLY public.cities ADD CONSTRAINT cities_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'comments_pkey') THEN ALTER TABLE ONLY public.comments ADD CONSTRAINT comments_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'contents_pkey') THEN ALTER TABLE ONLY public.contents ADD CONSTRAINT contents_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'countries_pkey') THEN ALTER TABLE ONLY public.countries ADD CONSTRAINT countries_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'districts_pkey') THEN ALTER TABLE ONLY public.districts ADD CONSTRAINT districts_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'drivers_phone_key') THEN ALTER TABLE ONLY public.drivers ADD CONSTRAINT drivers_phone_key UNIQUE (phone); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'drivers_pkey') THEN ALTER TABLE ONLY public.drivers ADD CONSTRAINT drivers_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'drivers_user_id_unique') THEN ALTER TABLE ONLY public.drivers ADD CONSTRAINT drivers_user_id_unique UNIQUE (user_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'form_submissions_pkey') THEN ALTER TABLE ONLY public.form_submissions ADD CONSTRAINT form_submissions_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'homepage_pkey') THEN ALTER TABLE ONLY public.homepage ADD CONSTRAINT homepage_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'locations_pkey') THEN ALTER TABLE ONLY public.locations ADD CONSTRAINT locations_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'mail_queue_pkey') THEN ALTER TABLE ONLY public.mail_queue ADD CONSTRAINT mail_queue_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'media_pkey') THEN ALTER TABLE ONLY public.media ADD CONSTRAINT media_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'password_resets_pkey') THEN ALTER TABLE ONLY public.password_resets ADD CONSTRAINT password_resets_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'permissions_name_key') THEN ALTER TABLE ONLY public.permissions ADD CONSTRAINT permissions_name_key UNIQUE (name); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'permissions_pkey') THEN ALTER TABLE ONLY public.permissions ADD CONSTRAINT permissions_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ride_fare_configs_city_code_key') THEN ALTER TABLE ONLY public.ride_fare_configs ADD CONSTRAINT ride_fare_configs_city_code_key UNIQUE (city_code); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ride_fare_configs_pkey') THEN ALTER TABLE ONLY public.ride_fare_configs ADD CONSTRAINT ride_fare_configs_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ride_offers_pkey') THEN ALTER TABLE ONLY public.ride_offers ADD CONSTRAINT ride_offers_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'rides_pkey') THEN ALTER TABLE ONLY public.rides ADD CONSTRAINT rides_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'role_permissions_pkey') THEN ALTER TABLE ONLY public.role_permissions ADD CONSTRAINT role_permissions_pkey PRIMARY KEY (role_id, permission_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'roles_name_key') THEN ALTER TABLE ONLY public.roles ADD CONSTRAINT roles_name_key UNIQUE (name); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'roles_pkey') THEN ALTER TABLE ONLY public.roles ADD CONSTRAINT roles_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'sessions_pkey') THEN ALTER TABLE ONLY public.sessions ADD CONSTRAINT sessions_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'settings_pkey') THEN ALTER TABLE ONLY public.settings ADD CONSTRAINT settings_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'terms_pkey') THEN ALTER TABLE ONLY public.terms ADD CONSTRAINT terms_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'timeline_events_pkey') THEN ALTER TABLE ONLY public.timeline_events ADD CONSTRAINT timeline_events_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'user_permissions_pkey') THEN ALTER TABLE ONLY public.user_permissions ADD CONSTRAINT user_permissions_pkey PRIMARY KEY (user_id, permission_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'user_roles_pkey') THEN ALTER TABLE ONLY public.user_roles ADD CONSTRAINT user_roles_pkey PRIMARY KEY (user_id, role_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_apple_id_key') THEN ALTER TABLE ONLY public.users ADD CONSTRAINT users_apple_id_key UNIQUE (apple_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_email_key') THEN ALTER TABLE ONLY public.users ADD CONSTRAINT users_email_key UNIQUE (email); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_google_id_key') THEN ALTER TABLE ONLY public.users ADD CONSTRAINT users_google_id_key UNIQUE (google_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_pkey') THEN ALTER TABLE ONLY public.users ADD CONSTRAINT users_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_username_key') THEN ALTER TABLE ONLY public.users ADD CONSTRAINT users_username_key UNIQUE (username); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'vocabularies_pkey') THEN ALTER TABLE ONLY public.vocabularies ADD CONSTRAINT vocabularies_pkey PRIMARY KEY (id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'vocabulary_categories_pkey') THEN ALTER TABLE ONLY public.vocabulary_categories ADD CONSTRAINT vocabulary_categories_pkey PRIMARY KEY (vocabulary_id, category_term_id); END IF; END $$;

DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx-comments-content') THEN CREATE INDEX "idx-comments-content" ON public.comments USING btree (content_type, content_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx-comments-user_id') THEN CREATE INDEX "idx-comments-user_id" ON public.comments USING btree (user_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx-form_submissions-created_at') THEN CREATE INDEX "idx-form_submissions-created_at" ON public.form_submissions USING btree (created_at); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx-form_submissions-form_id') THEN CREATE INDEX "idx-form_submissions-form_id" ON public.form_submissions USING btree (form_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx-password_resets-token') THEN CREATE INDEX "idx-password_resets-token" ON public.password_resets USING btree (token); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_contents_content_type') THEN CREATE INDEX idx_contents_content_type ON public.contents USING btree (content_type); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_contents_deleted_at') THEN CREATE INDEX idx_contents_deleted_at ON public.contents USING btree (deleted_at); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_contents_order_id') THEN CREATE INDEX idx_contents_order_id ON public.contents USING btree (order_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_contents_parent_id') THEN CREATE INDEX idx_contents_parent_id ON public.contents USING btree (parent_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_contents_publish') THEN CREATE INDEX idx_contents_publish ON public.contents USING btree (publish); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_contents_type_order') THEN CREATE INDEX idx_contents_type_order ON public.contents USING btree (content_type, order_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_contents_type_publish') THEN CREATE INDEX idx_contents_type_publish ON public.contents USING btree (content_type, publish) WHERE (deleted_at IS NULL); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_drivers_is_online') THEN CREATE INDEX idx_drivers_is_online ON public.drivers USING btree (is_online); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_drivers_location') THEN CREATE INDEX idx_drivers_location ON public.drivers USING btree (current_lat, current_lon) WHERE ((is_online = true) AND (is_active = true)); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_locations_active') THEN CREATE INDEX idx_locations_active ON public.locations USING btree (is_active) WHERE (is_active = true); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_locations_category') THEN CREATE INDEX idx_locations_category ON public.locations USING btree (category) WHERE (is_active = true); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_locations_name_trgm') THEN CREATE INDEX idx_locations_name_trgm ON public.locations USING gin (name public.gin_trgm_ops); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_mail_queue_scheduled') THEN CREATE INDEX idx_mail_queue_scheduled ON public.mail_queue USING btree (scheduled_at); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_mail_queue_status') THEN CREATE INDEX idx_mail_queue_status ON public.mail_queue USING btree (status); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_mail_queue_template_name') THEN CREATE INDEX idx_mail_queue_template_name ON public.mail_queue USING btree (template_name); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_media_content') THEN CREATE INDEX idx_media_content ON public.media USING btree (content_type, content_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_ride_offers_driver_ride') THEN CREATE INDEX idx_ride_offers_driver_ride ON public.ride_offers USING btree (driver_id, ride_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_ride_offers_ride_id') THEN CREATE INDEX idx_ride_offers_ride_id ON public.ride_offers USING btree (ride_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_rides_driver_id') THEN CREATE INDEX idx_rides_driver_id ON public.rides USING btree (driver_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_rides_status') THEN CREATE INDEX idx_rides_status ON public.rides USING btree (status); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_rides_user_id') THEN CREATE INDEX idx_rides_user_id ON public.rides USING btree (user_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_sessions_expires_at') THEN CREATE INDEX idx_sessions_expires_at ON public.sessions USING btree (expires_at); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_sessions_user_id') THEN CREATE INDEX idx_sessions_user_id ON public.sessions USING btree (user_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_terms_order_id') THEN CREATE INDEX idx_terms_order_id ON public.terms USING btree (order_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_terms_parent') THEN CREATE INDEX idx_terms_parent ON public.terms USING btree (parent_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_terms_publish') THEN CREATE INDEX idx_terms_publish ON public.terms USING btree (publish); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_terms_vocabulary') THEN CREATE INDEX idx_terms_vocabulary ON public.terms USING btree (vocabulary_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_timeline_created_at') THEN CREATE INDEX idx_timeline_created_at ON public.timeline_events USING btree (created_at); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_timeline_module_content') THEN CREATE INDEX idx_timeline_module_content ON public.timeline_events USING btree (module_type, content_type, content_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_timeline_public') THEN CREATE INDEX idx_timeline_public ON public.timeline_events USING btree (is_public); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_timeline_user') THEN CREATE INDEX idx_timeline_user ON public.timeline_events USING btree (user_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_users_guest_session_id') THEN CREATE INDEX idx_users_guest_session_id ON public.users USING btree (guest_session_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_users_is_guest') THEN CREATE INDEX idx_users_is_guest ON public.users USING btree (is_guest); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_vocabularies_order_id') THEN CREATE INDEX idx_vocabularies_order_id ON public.vocabularies USING btree (order_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_vocabularies_type') THEN CREATE INDEX idx_vocabularies_type ON public.vocabularies USING btree (vocabulary_type); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_vocabulary_categories_category') THEN CREATE INDEX idx_vocabulary_categories_category ON public.vocabulary_categories USING btree (category_term_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_vocabulary_categories_vocabulary') THEN CREATE INDEX idx_vocabulary_categories_vocabulary ON public.vocabulary_categories USING btree (vocabulary_id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'idx_rides_active_driver') THEN CREATE INDEX idx_rides_active_driver ON public.rides USING btree (driver_id, status) WHERE ((driver_id IS NOT NULL) AND (status = ANY (ARRAY['accepted'::public.ride_status, 'picked_up'::public.ride_status]))); END IF; END $$;

DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'drivers_user_id_fkey') THEN ALTER TABLE ONLY public.drivers ADD CONSTRAINT drivers_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE SET NULL; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk-comments-user_id') THEN ALTER TABLE ONLY public.comments ADD CONSTRAINT "fk-comments-user_id" FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk-form_submissions-form_id') THEN ALTER TABLE ONLY public.form_submissions ADD CONSTRAINT "fk-form_submissions-form_id" FOREIGN KEY (form_id) REFERENCES public.contents(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk-form_submissions-user_id') THEN ALTER TABLE ONLY public.form_submissions ADD CONSTRAINT "fk-form_submissions-user_id" FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE SET NULL; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk-password_resets-user_id') THEN ALTER TABLE ONLY public.password_resets ADD CONSTRAINT "fk-password_resets-user_id" FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_cities_country_id') THEN ALTER TABLE ONLY public.cities ADD CONSTRAINT fk_cities_country_id FOREIGN KEY (country_id) REFERENCES public.countries(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_content_terms_content') THEN ALTER TABLE ONLY public.content_terms ADD CONSTRAINT fk_content_terms_content FOREIGN KEY (content_id) REFERENCES public.contents(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_content_terms_term') THEN ALTER TABLE ONLY public.content_terms ADD CONSTRAINT fk_content_terms_term FOREIGN KEY (term_id) REFERENCES public.terms(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_contents_children') THEN ALTER TABLE ONLY public.contents ADD CONSTRAINT fk_contents_children FOREIGN KEY (parent_id) REFERENCES public.contents(id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_contents_user_id') THEN ALTER TABLE ONLY public.contents ADD CONSTRAINT fk_contents_user_id FOREIGN KEY (user_id) REFERENCES public.users(id) ON UPDATE CASCADE ON DELETE SET NULL; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_districts_city_id') THEN ALTER TABLE ONLY public.districts ADD CONSTRAINT fk_districts_city_id FOREIGN KEY (city_id) REFERENCES public.cities(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_terms_parent') THEN ALTER TABLE ONLY public.terms ADD CONSTRAINT fk_terms_parent FOREIGN KEY (parent_id) REFERENCES public.terms(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_terms_vocabulary') THEN ALTER TABLE ONLY public.terms ADD CONSTRAINT fk_terms_vocabulary FOREIGN KEY (vocabulary_id) REFERENCES public.vocabularies(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ride_offers_driver_id_fkey') THEN ALTER TABLE ONLY public.ride_offers ADD CONSTRAINT ride_offers_driver_id_fkey FOREIGN KEY (driver_id) REFERENCES public.drivers(id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'ride_offers_ride_id_fkey') THEN ALTER TABLE ONLY public.ride_offers ADD CONSTRAINT ride_offers_ride_id_fkey FOREIGN KEY (ride_id) REFERENCES public.rides(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'rides_driver_id_fkey') THEN ALTER TABLE ONLY public.rides ADD CONSTRAINT rides_driver_id_fkey FOREIGN KEY (driver_id) REFERENCES public.drivers(id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'rides_user_id_fkey') THEN ALTER TABLE ONLY public.rides ADD CONSTRAINT rides_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id); END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'role_permissions_permission_id_fkey') THEN ALTER TABLE ONLY public.role_permissions ADD CONSTRAINT role_permissions_permission_id_fkey FOREIGN KEY (permission_id) REFERENCES public.permissions(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'role_permissions_role_id_fkey') THEN ALTER TABLE ONLY public.role_permissions ADD CONSTRAINT role_permissions_role_id_fkey FOREIGN KEY (role_id) REFERENCES public.roles(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'user_permissions_permission_id_fkey') THEN ALTER TABLE ONLY public.user_permissions ADD CONSTRAINT user_permissions_permission_id_fkey FOREIGN KEY (permission_id) REFERENCES public.permissions(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'user_permissions_user_id_fkey') THEN ALTER TABLE ONLY public.user_permissions ADD CONSTRAINT user_permissions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'user_roles_role_id_fkey') THEN ALTER TABLE ONLY public.user_roles ADD CONSTRAINT user_roles_role_id_fkey FOREIGN KEY (role_id) REFERENCES public.roles(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'user_roles_user_id_fkey') THEN ALTER TABLE ONLY public.user_roles ADD CONSTRAINT user_roles_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'vocabulary_categories_category_term_id_fkey') THEN ALTER TABLE ONLY public.vocabulary_categories ADD CONSTRAINT vocabulary_categories_category_term_id_fkey FOREIGN KEY (category_term_id) REFERENCES public.terms(id) ON DELETE CASCADE; END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'vocabulary_categories_vocabulary_id_fkey') THEN ALTER TABLE ONLY public.vocabulary_categories ADD CONSTRAINT vocabulary_categories_vocabulary_id_fkey FOREIGN KEY (vocabulary_id) REFERENCES public.vocabularies(id) ON DELETE CASCADE; END IF; END $$;
