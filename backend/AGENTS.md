# Agent Instructions

## Project
Rust Axum web application (v2.1.27) with SeaORM + PostgreSQL. Workspace with root crate + `migration` crate.

## Theme Customizations (SGM Theme)
### Recent UI Changes (2026-05)
- **Categories sidebar**: Hierarchical 4-level collapsible menu in `templates/sgm/home/categories.html`
- **Product grids**: Uniform image display (`object-fit: contain`) in `product_grid.html`, `product_grid_yatay.html`, `product_list.html`
- **Slider**: Bottom-right navigation, dark controls, mobile responsive in `col_image_slider.html`
- **Section titles**: Bold (800 weight) with red gradient underline in `static/css/base.css`
- **Badges**: Red discount, grey variants, blue color badges in product cards
- **Features strip**: 2-column mobile / 4-column desktop in `features_strip.html`
- **Category cover**: Square image containers with `object-fit: contain` in `category_cover.html`
- **Mobile**: Sidebar hidden on mobile (`d-none d-lg-block`), slider full-width

### Theme Files Location
- Templates: `templates/sgm/home/sections/`, `templates/sgm/pages/`
- CSS: `templates/sgm/static/css/base.css`
- Navbar: `templates/sgm/_partials/navbar.html`

## Commands
```bash
# Run server (reads config.toml)
cargo run

# Print version
cargo run -- --version

# Run with custom database
DATABASE_URL="postgresql://user:pass@host/db" cargo run

# Migrations
DATABASE_URL="postgresql://user:pass@host/db" sea-orm-cli migrate up
DATABASE_URL="postgresql://user:pass@host/db" sea-orm-cli migrate status

# Generate migration (from migration crate directory)
cd migration && cargo run -- generate <name>

# Generate entity from existing DB
sea-orm-cli generate entity -u $DATABASE_URL -o src/entities --tables <table>
```

## Config
- `config.toml` - Server, database, templates, OAuth, JWT settings
- No `.env` files - config is committed (review before committing secrets)

## Architecture
- **Entry**: `src/main.rs` - creates AppState, runs migrations, starts Axum router
- **Modules**: `src/modules/{auth,ecommerce,content,taxonomy,media,mailer,payment_provider,b2b,bookmarks,timeline,search,form,admin}/`
- **Database**: SeaORM entities in `migration/src/entities/`
- **Templates**: Tera templates in `templates/{theme}/**`
- **i18n**: `locales/*.yml` files, loaded at compile-time via `rust_i18n::i18n!("locales")`

## Testing
- No test suite exists - verify manually
- Templates and configs are checked at startup (see main.rs:204-208)

## Gotchas
- Admin templates embedded at build time via `rust-embed` - changes require rebuild
- Template hot reload controlled by `config.toml: template.hot_reload`
- Session config: `config.toml: session.max_age` and `session.secret_key`
- OAuth secrets in `config.toml: oauth.google` (committed - rotate before production)
- JWT: `config.toml: jwt.secret`, `jwt.access_token_expiry`, `jwt.refresh_token_expiry`

## Docs
Detailed guides in `documents/`:
- AUTH_GUIDE.md, RBAC_V2_GUIDE.md
- B2B_COMPLETE_GUIDE.md, B2B_IMPLEMENTATION_GUIDE.md
- PAYMENT_PROVIDER_GUIDE.md
- MAIL_SYSTEM_IMPLEMENTATION.md
- PRODUCT_MANAGEMENT_GUIDE.md