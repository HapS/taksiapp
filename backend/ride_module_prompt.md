# Ride Modülü — Agent Prompt

Sen bir Rust/Axum backend geliştiricisin. Mevcut bir projeye taksi uygulaması modülü ekleyeceksin. Aşağıdaki tüm talimatları eksiksiz uygula.

---

## Proje yapısı

```
src/
├── modules/
│   ├── background_tasks/   ← mevcut, buraya location flush eklenecek
│   ├── ride/               ← YENİ modül, sen oluşturacaksın
│   └── ... (diğer mevcut modüller, dokunma)
├── app_state.rs            ← Hub ve Redis eklenecek
└── main.rs                 ← Redis init ve route eklenecek
```

---

## Görev 1 — Migration dosyası

`migration/` klasöründe mevcut migration dosyalarına bakarak numbering convention'ı anla ve yeni bir migration dosyası oluştur. Aşağıdaki SQL'i uygula:

```sql
-- ENUM tipleri
CREATE TYPE ride_status AS ENUM (
    'searching', 'accepted', 'picked_up', 'completed', 'cancelled', 'no_driver'
);

CREATE TYPE offer_status AS ENUM (
    'pending', 'accepted', 'rejected', 'timeout'
);

-- Sürücüler tablosu
CREATE TABLE IF NOT EXISTS drivers (
    id                  BIGSERIAL PRIMARY KEY,
    user_id             BIGINT REFERENCES users(id) ON DELETE SET NULL,
    full_name           VARCHAR NOT NULL,
    phone               VARCHAR NOT NULL UNIQUE,
    vehicle_plate       VARCHAR NOT NULL,
    vehicle_model       VARCHAR NOT NULL,
    rating              DOUBLE PRECISION NOT NULL DEFAULT 5.0,
    is_active           BOOLEAN NOT NULL DEFAULT true,
    is_online           BOOLEAN NOT NULL DEFAULT false,
    current_lat         DOUBLE PRECISION,
    current_lon         DOUBLE PRECISION,
    location_updated_at TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_drivers_is_online ON drivers(is_online);
CREATE INDEX idx_drivers_location ON drivers(current_lat, current_lon)
    WHERE is_online = true AND is_active = true;

-- Yolculuklar tablosu
CREATE TABLE IF NOT EXISTS rides (
    id              BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    driver_id       BIGINT REFERENCES drivers(id),
    status          ride_status NOT NULL DEFAULT 'searching',
    pickup_lat      DOUBLE PRECISION NOT NULL,
    pickup_lon      DOUBLE PRECISION NOT NULL,
    pickup_address  TEXT NOT NULL,
    dropoff_lat     DOUBLE PRECISION NOT NULL,
    dropoff_lon     DOUBLE PRECISION NOT NULL,
    dropoff_address TEXT NOT NULL,
    distance_km     DOUBLE PRECISION,
    duration_sec    INTEGER,
    fare_amount     NUMERIC(10, 2),
    requested_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    accepted_at     TIMESTAMPTZ,
    picked_up_at    TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    cancelled_at    TIMESTAMPTZ
);

CREATE INDEX idx_rides_user_id  ON rides(user_id);
CREATE INDEX idx_rides_driver_id ON rides(driver_id);
CREATE INDEX idx_rides_status   ON rides(status);

-- Teklif geçmişi tablosu
CREATE TABLE IF NOT EXISTS ride_offers (
    id           BIGSERIAL PRIMARY KEY,
    ride_id      BIGINT NOT NULL REFERENCES rides(id) ON DELETE CASCADE,
    driver_id    BIGINT NOT NULL REFERENCES drivers(id),
    status       offer_status NOT NULL DEFAULT 'pending',
    offer_order  INTEGER NOT NULL,
    offered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    responded_at TIMESTAMPTZ
);

CREATE INDEX idx_ride_offers_ride_id ON ride_offers(ride_id);
```

---

## Görev 2 — SeaORM entity'leri

`src/modules/ride/entities/` altında şu dosyaları oluştur:

### `drivers.rs`
- `id`: `i64`
- `user_id`: `Option<i64>`
- `is_online`, `is_active`: `bool`
- `current_lat`, `current_lon`: `Option<f64>`
- Relation: `has_many rides`, `has_many ride_offers`

### `rides.rs`
- `RideStatus` enum'u `DeriveActiveEnum` ile: `searching`, `accepted`, `picked_up`, `completed`, `cancelled`, `no_driver`
- `user_id`: `i64`, `driver_id`: `Option<i64>`
- `fare_amount`: `Option<Decimal>`
- Relation: `belongs_to users`, `belongs_to drivers`, `has_many ride_offers`

### `ride_offers.rs`
- `OfferStatus` enum'u: `pending`, `accepted`, `rejected`, `timeout`
- `offer_order`: `i32`
- Relation: `belongs_to rides`, `belongs_to drivers`

### `mod.rs`
Üç modülü `pub mod` ile export et.

---

## Görev 3 — WebSocket Hub

### `src/modules/ride/ws/hub.rs`

```rust
pub type Tx = tokio::sync::mpsc::UnboundedSender<String>;

pub struct DriverSession {
    pub tx:  Tx,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub struct PassengerSession {
    pub tx: Tx,
}

pub struct Hub {
    pub drivers:    Arc<DashMap<i64, DriverSession>>,
    pub passengers: Arc<DashMap<i64, PassengerSession>>,
    // ride_id → (driver_id, user_id)
    pub ride_rooms: Arc<DashMap<i64, (i64, i64)>>,
}
```

Metodlar:
- `new() -> Self`
- `send_to_driver(&self, driver_id: i64, msg: &ServerMessage)`
- `send_to_passenger(&self, passenger_id: i64, msg: &ServerMessage)`
- `broadcast_driver_location(&self, ride_id: i64, lat: f64, lon: f64)`

### `src/modules/ride/ws/messages.rs`

```rust
// ClientMessage — serde tag = "type", rename_all = "snake_case"
pub enum ClientMessage {
    LocationUpdate { lat: f64, lon: f64 },
    OfferResponse  { ride_id: i64, accepted: bool },
    Ping,
}

// ServerMessage — serde tag = "type", rename_all = "snake_case"
pub enum ServerMessage {
    RideOffer {
        ride_id:         i64,
        pickup_address:  String,
        dropoff_address: String,
        distance_km:     f64,
        fare_amount:     f64,
        expires_in_secs: u32,
    },
    DriverLocation      { ride_id: i64, lat: f64, lon: f64 },
    RideStatusChanged   { ride_id: i64, status: String },
    OfferExpired        { ride_id: i64 },
    Pong,
    Error               { message: String },
}
```

### `src/modules/ride/ws/handler.rs`

İki public async fonksiyon:

**`driver_ws_handler`**
- Query param: `driver_id: i64` (TODO: JWT ile değiştirilecek)
- Bağlantı kurulunca: hub'a `DriverSession` ekle, DB'de `is_online = true` yap
- Gelen mesajlar:
  - `LocationUpdate` → hub session güncelle + Redis'e yaz (`driver:{id}:location` = `"{lat},{lon}"`) + aktif ride varsa `broadcast_driver_location`
  - `OfferResponse` → `ride_offers` kaydını güncelle; kabul edildiyse `rides.driver_id` ve `status = accepted`, `accepted_at = NOW()` güncelle
  - `Ping` → `Pong` gönder
- Bağlantı kesilince: hub'dan çıkar, `is_online = false` yap

**`passenger_ws_handler`**
- Query param: `user_id: i64` (TODO: JWT ile değiştirilecek)
- Hub'a `PassengerSession` ekle
- Gelen mesaj: sadece `Ping` → `Pong`
- Bağlantı kesilince: hub'dan çıkar

### `src/modules/ride/ws/mod.rs`
`pub mod handler`, `pub mod hub`, `pub mod messages`

---

## Görev 4 — Dispatch service

### `src/modules/ride/dispatch.rs`

`pub async fn dispatch_ride(state: Arc<AppState>, ride_id: i64)`

Adımlar:

1. `rides` tablosundan ride'ı çek
2. Raw SQL ile yakın aktif sürücüleri bul (PostGIS yok, Euclidean kullan):

```sql
SELECT id FROM drivers
WHERE is_online = true
  AND is_active = true
  AND current_lat IS NOT NULL
  AND current_lon IS NOT NULL
  AND ABS(current_lat - $1) < 0.05
  AND ABS(current_lon - $2) < 0.05
  AND id NOT IN (
      SELECT driver_id FROM ride_offers WHERE ride_id = $3
  )
ORDER BY (ABS(current_lat - $1) + ABS(current_lon - $2)) ASC
LIMIT 5
```

3. Her sürücü için sırayla:
   - `ride_offers` insert et (`status: pending`, `offer_order: N`)
   - Hub üzerinden `ServerMessage::RideOffer` gönder
   - `tokio::time::sleep(Duration::from_secs(30))` bekle
   - `ride_offers` tablosundan son durumu kontrol et
   - `accepted` → döngüyü bitir
   - `pending` → `timeout` olarak güncelle, sonraki sürücüye geç

4. Tüm sürücüler geçildiyse:
   - `rides.status = no_driver` yap
   - Hub üzerinden yolcuya `RideStatusChanged { status: "no_driver" }` gönder

Tüm DB hataları `tracing::error!` ile logla, panic yapma.

---

## Görev 5 — HTTP controller

### `src/modules/ride/controllers/ride.rs`

**`POST /api/ride/request`**

Request body:
```rust
pub struct RideRequest {
    pub user_id:         i64,  // TODO: JWT'den al
    pub pickup_lat:      f64,
    pub pickup_lon:      f64,
    pub pickup_address:  String,
    pub dropoff_lat:     f64,
    pub dropoff_lon:     f64,
    pub dropoff_address: String,
}
```

Adımlar:
1. `rides` tablosuna insert et (`status: searching`)
2. ORS'den mesafe/süre al:
   - `state.config` içinde `ors_api_key` ve `ors_base_url` yoksa TODO yorum ekle, `None` bırak
   - Endpoint: `GET {ors_base_url}/v2/directions/driving-car?api_key={key}&start={lon},{lat}&end={lon},{lat}`
   - `reqwest` ile çağır, başarısız olursa hata verme, `None` bırak
   - Başarılıysa `rides.distance_km` ve `rides.duration_sec` güncelle
3. `tokio::spawn` ile `dispatch::dispatch_ride(state, ride_id)` başlat
4. Response: `{ "ride_id": i64, "status": "searching" }`

**`GET /api/ride/:id`**

- `rides` tablosundan kaydı çek
- `driver_id` varsa `drivers` tablosundan `full_name`, `vehicle_plate`, `vehicle_model`, `phone` join'le
- JSON response dön

### `src/modules/ride/controllers/mod.rs`
`pub mod ride`

---

## Görev 6 — Background task (location flush)

`src/modules/background_tasks/` klasöründeki mevcut dosyaları incele. Aynı pattern'i kullanarak location flush'ı ekle.

Her 30 saniyede bir:
1. `hub.drivers` map'ini iterate et
2. `lat` ve `lon` değeri `Some` olan sürücüleri topla
3. Her biri için ayrı UPDATE at (`tokio::join_all` ile paralel):

```sql
UPDATE drivers
SET current_lat = $1, current_lon = $2, location_updated_at = NOW()
WHERE id = $3
```

---

## Görev 7 — Routes

### `src/modules/ride/routes.rs`

```rust
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/ride/request", post(controllers::ride::request_ride))
        .route("/api/ride/:id",     get(controllers::ride::get_ride))
        .route("/ws/driver",        get(ws::handler::driver_ws_handler))
        .route("/ws/passenger",     get(ws::handler::passenger_ws_handler))
}
```

---

## Görev 8 — Modül dosyaları

### `src/modules/ride/mod.rs`
```rust
pub mod controllers;
pub mod dispatch;
pub mod entities;
pub mod routes;
pub mod ws;
```

### `src/modules/mod.rs`
Mevcut dosyaya `pub mod ride;` satırını ekle.

---

## Görev 9 — AppState güncellemesi

`src/app_state.rs` dosyasında:

1. Import ekle:
```rust
use crate::modules::ride::ws::hub::Hub;
```

2. `AppState` struct'ına ekle:
```rust
pub hub:   std::sync::Arc<Hub>,
pub redis: std::sync::Arc<redis::aio::ConnectionManager>,
```

3. `new()` imzasına ekle:
```rust
redis: redis::aio::ConnectionManager,
```

4. `Self { ... }` bloğuna ekle:
```rust
hub:   std::sync::Arc::new(Hub::new()),
redis: std::sync::Arc::new(redis),
```

---

## Görev 10 — main.rs güncellemesi

1. DB bağlantısından hemen sonra Redis ekle:
```rust
let redis_url = std::env::var("REDIS_URL")
    .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
let redis_client = redis::Client::open(redis_url.as_str())?;
let redis_manager = redis::aio::ConnectionManager::new(redis_client).await?;
println!("🍀 Redis bağlantısı kuruldu");
```

2. `AppState::new(...)` çağrısına son parametre olarak `redis_manager` ekle.

3. `modules::background_tasks::start_all(...)` satırından sonra, mevcut pattern ile location flush'ı başlat.

4. Router'daki diğer `.merge()` satırlarına ekle:
```rust
.merge(modules::ride::routes::routes())
```

---

## Görev 11 — Cargo.toml

Aşağıdakilerin eklendiğinden emin ol (zaten varsa ekleme):

```toml
redis   = { version = "0.25", features = ["tokio-comp", "connection-manager"] }
futures = "0.3"
reqwest = { version = "0.11", features = ["json"] }
```

> `dashmap` zaten mevcut projede var, ekleme.

---

## Kurallar

- Mevcut hiçbir dosyaya zarar verme, sadece belirtilen satırları ekle
- Tüm yeni kodlar `src/modules/ride/` altında olacak; istisna `app_state.rs` ve `main.rs`
- SeaORM entity'lerinde `id` alanları `i64` (mevcut `users` tablosu `bigint` kullanıyor)
- Kullanılmayan import'lar olmasın — `cargo check` geçmeli
- Panic kullanma; tüm hatalar `Result` veya `Option` ile handle edilmeli
- Dispatch'teki DB hataları `tracing::error!` ile loglanmalı
- Eksik olan şeyler için `// TODO:` yorumu ekle (JWT, PostGIS vs.)
