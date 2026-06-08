# Sürücü Yarım Kalan Yolculuk Restore + Bot JWT — Agent Prompt

## Mevcut durum

- Backend: `POST /api/ride/:id/status`, `POST /api/ride/:id/cancel`, `GET /api/ride/:id` mevcut
- Backend JWT: `JwtClaims` extractor çalışıyor, `claims.user_id` → `i64`
- Flutter `driver_home_page.dart`: `initState` sadece `_initLocation()` çağırıyor, aktif ride kontrolü yok
- `RideService`: `updateRideStatus()`, `cancelRide()` metodları mevcut
- Bot: `WS_BASE_URL = "wss://one.web.tr/ws/driver?driver_id="` — JWT yok, driver_id query param

---

## BÖLÜM 1 — BACKEND

### Görev 1 — `GET /api/ride/driver/active` endpoint'i ekle

**Dosya:** `src/modules/ride/controllers/ride.rs`

Bu endpoint sürücünün JWT'sinden `user_id`'yi alır, bu `user_id`'ye ait `drivers` kaydını bulur, o sürücünün `accepted` veya `picked_up` durumundaki ride'ını döndürür.

```rust
pub async fn get_driver_active_ride(
    claims: JwtClaims,
    State(state): State<AppState>,
) -> impl IntoResponse {
    use crate::modules::ride::entities::drivers::{self, Entity as Driver};
    use sea_orm::{ColumnTrait, QueryFilter};

    // 1. user_id → driver kaydını bul
    let driver = match Driver::find()
        .filter(drivers::Column::UserId.eq(claims.user_id))
        .filter(drivers::Column::IsActive.eq(true))
        .one(&state.db)
        .await
    {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (StatusCode::OK, Json(serde_json::json!({
                "active_ride": null
            }))).into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "get_driver_active_ride: driver sorgusu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "internal error"
            }))).into_response();
        }
    };

    // 2. Bu sürücünün accepted/picked_up ride'ını bul
    use crate::modules::ride::entities::rides::{self, Entity as Ride, RideStatus};
    use sea_orm::Condition;

    let active_ride = match Ride::find()
        .filter(rides::Column::DriverId.eq(driver.id))
        .filter(
            Condition::any()
                .add(rides::Column::Status.eq(RideStatus::Accepted))
                .add(rides::Column::Status.eq(RideStatus::PickedUp)),
        )
        .one(&state.db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "get_driver_active_ride: ride sorgusu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "internal error"
            }))).into_response();
        }
    };

    match active_ride {
        None => {
            (StatusCode::OK, Json(serde_json::json!({
                "active_ride": null
            }))).into_response()
        }
        Some(ride) => {
            (StatusCode::OK, Json(serde_json::json!({
                "active_ride": {
                    "ride_id": ride.id,
                    "status": ride.status.as_str(),
                    "pickup_address": ride.pickup_address,
                    "dropoff_address": ride.dropoff_address,
                    "pickup_lat": ride.pickup_lat,
                    "pickup_lon": ride.pickup_lon,
                    "dropoff_lat": ride.dropoff_lat,
                    "dropoff_lon": ride.dropoff_lon,
                    "distance_km": ride.distance_km,
                    "duration_sec": ride.duration_sec,
                    "requested_at": ride.requested_at.to_rfc3339(),
                }
            }))).into_response()
        }
    }
}
```

---

### Görev 2 — Route'a ekle

**Dosya:** `src/modules/ride/routes.rs`

```rust
.route("/api/ride/driver/active", get(controllers::ride::get_driver_active_ride))
```

> **ÖNEMLİ:** Bu route'u `/api/ride/:id` route'undan **önce** ekle. Aksi halde Axum `"driver"` string'ini `id` parametresi olarak parse etmeye çalışır.

---

## BÖLÜM 2 — FLUTTER

### Görev 3 — `RideService`'e aktif ride sorgulama ekle

**Dosya:** `lib/modules/ride_sharing/services/ride_service.dart`

`ActiveRideInfo` modeli ekle:

```dart
class ActiveRideInfo {
  final int rideId;
  final String status;
  final String pickupAddress;
  final String dropoffAddress;
  final double pickupLat;
  final double pickupLon;
  final double dropoffLat;
  final double dropoffLon;
  final double? distanceKm;
  final int? durationSec;

  ActiveRideInfo({
    required this.rideId,
    required this.status,
    required this.pickupAddress,
    required this.dropoffAddress,
    required this.pickupLat,
    required this.pickupLon,
    required this.dropoffLat,
    required this.dropoffLon,
    this.distanceKm,
    this.durationSec,
  });

  factory ActiveRideInfo.fromJson(Map<String, dynamic> json) {
    return ActiveRideInfo(
      rideId: json['ride_id'] as int,
      status: json['status'] as String,
      pickupAddress: json['pickup_address'] as String,
      dropoffAddress: json['dropoff_address'] as String,
      pickupLat: (json['pickup_lat'] as num).toDouble(),
      pickupLon: (json['pickup_lon'] as num).toDouble(),
      dropoffLat: (json['dropoff_lat'] as num).toDouble(),
      dropoffLon: (json['dropoff_lon'] as num).toDouble(),
      distanceKm: (json['distance_km'] as num?)?.toDouble(),
      durationSec: json['duration_sec'] as int?,
    );
  }
}
```

`getDriverActiveRide()` metodu ekle:

```dart
/// Sürücünün aktif (accepted/picked_up) yolculuğunu döndürür.
/// Yoksa null döner.
/// Backend: GET /api/ride/driver/active (JWT Auth gerekli)
static Future<ActiveRideInfo?> getDriverActiveRide() async {
  try {
    final token = await AuthService().getAccessToken();
    if (token == null) return null;

    final url = Uri.parse('${AppConfig.apiEndpoint}/ride/driver/active');
    final response = await http.get(
      url,
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer $token',
      },
    );

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body) as Map<String, dynamic>;
      final activeRide = data['active_ride'];
      if (activeRide == null) return null;
      return ActiveRideInfo.fromJson(activeRide as Map<String, dynamic>);
    }
    return null;
  } catch (e) {
    debugPrint('RideService.getDriverActiveRide error: $e');
    return null;
  }
}
```

---

### Görev 4 — `DriverHomePage.initState` güncelle

**Dosya:** `lib/modules/ride_sharing/driver_home_page.dart`

`initState`'i güncelle:

```dart
@override
void initState() {
  super.initState();
  _initLocation();
  // Uygulama açılınca yarım kalan yolculuk kontrolü yap
  WidgetsBinding.instance.addPostFrameCallback((_) {
    _checkAndRestoreActiveRide();
  });
}
```

---

### Görev 5 — `_checkAndRestoreActiveRide()` metodu ekle

**Dosya:** `lib/modules/ride_sharing/driver_home_page.dart`

`awesome_dialog` zaten `pubspec.yaml`'da var. Import ekle (yoksa):

```dart
import 'package:awesome_dialog/awesome_dialog.dart';
import 'services/ride_service.dart';
```

Metodu ekle:

```dart
/// Uygulama açılınca backend'den aktif ride kontrolü yapar.
/// Varsa sürücüye sorar: Tamamlandı mı, İptal mi?
Future<void> _checkAndRestoreActiveRide() async {
  final activeRide = await RideService.getDriverActiveRide();
  if (activeRide == null) return;
  if (!mounted) return;

  debugPrint('DriverHomePage: Yarım kalan ride bulundu: #${activeRide.rideId}');

  // Dialog göstermeden önce kısa bekle (UI hazır olsun)
  await Future.delayed(const Duration(milliseconds: 500));
  if (!mounted) return;

  AwesomeDialog(
    context: context,
    dialogType: DialogType.warning,
    animType: AnimType.scale,
    title: 'Yarım Kalan Yolculuk',
    desc: '${activeRide.pickupAddress} → ${activeRide.dropoffAddress}\n\n'
        'Bu yolculuğu nasıl sonuçlandırmak istersiniz?',
    // Sürücü kaçamasın
    dismissOnTouchOutside: false,
    dismissOnBackKeyPress: false,
    btnOkText: 'Tamamlandı ✓',
    btnCancelText: 'İptal Et ✕',
    btnOkColor: Colors.green,
    btnCancelColor: Colors.red,
    btnOkOnPress: () async {
      await _resolveStaleRide(activeRide.rideId, 'completed');
    },
    btnCancelOnPress: () async {
      await _resolveStaleRide(activeRide.rideId, 'cancelled');
    },
  ).show();
}

/// Yarım kalan ride'ı çözümler ve UI'ı temizler.
Future<void> _resolveStaleRide(int rideId, String resolution) async {
  debugPrint('DriverHomePage: Stale ride #$rideId → $resolution');

  bool ok = false;
  if (resolution == 'completed') {
    final result = await RideService.updateRideStatus(rideId, 'completed');
    ok = result.success;
  } else {
    final result = await RideService.cancelRide(rideId, by: 'driver');
    ok = result.success;
  }

  if (!mounted) return;

  if (ok) {
    debugPrint('DriverHomePage: Stale ride çözümlendi');
    // State'i temizle — sürücü normal akışa geçer
    setState(() {
      _hasActiveRide = false;
      _activeRideInfo = null;
      _ridePhase = 'idle';
      _routeToPickup = [];
      _routeToDropoff = [];
    });
  } else {
    // Güncelleme başarısız — tekrar sor
    if (!mounted) return;
    AwesomeDialog(
      context: context,
      dialogType: DialogType.error,
      title: 'Hata',
      desc: 'Yolculuk güncellenemedi. Lütfen internet bağlantınızı kontrol edin.',
      btnOkText: 'Tekrar Dene',
      btnOkOnPress: () => _checkAndRestoreActiveRide(),
      dismissOnTouchOutside: false,
    ).show();
  }
}
```

---

### Görev 6 — `RideService.cancelRide` imzasını kontrol et

**Dosya:** `lib/modules/ride_sharing/services/ride_service.dart`

`cancelRide` metodunun `by` parametresini kabul ettiğinden emin ol. Şu an şöyle olmalı:

```dart
static Future<CancelRideResponse> cancelRide(int rideId, {String by = 'driver'}) async {
  // ...
  final body = {'by': by};
  // ...
}
```

`by` parametresi yoksa ekle.

---

## BÖLÜM 3 — BOT: JWT ile Bağlantı

### Görev 7 — `fake_driver_bot.py` güncelle: login + JWT WS bağlantısı

Mevcut bot `driver_id` query param kullanıyor. JWT devreye girdiğine göre bot da login olup token almalı.

**Dosya:** `fake_driver_bot.py`

Dosyanın tamamını şu şekilde yeniden yaz:

```python
#!/usr/bin/env python3
"""
Fake Sürücü Botu — JWT Auth versiyonu
- Backend'e login olur, JWT access token alır
- WS bağlantısını token ile kurar: /ws/driver?token=<jwt>
- Gelen teklifleri kabul eder, pickup → picked_up → dropoff → completed akışını yapar
- Bağlantı koparsa yeniden login edip bağlanır
"""

import asyncio
import json
import math
import signal
import sys
import urllib.request
import urllib.error
import urllib.parse
import websockets

# --- Ayarlar ---
API_BASE_URL = "https://one.web.tr/api"
WS_BASE_URL  = "wss://one.web.tr"

# (username, password, başlangıç_lat, başlangıç_lon)
# Bu kullanıcıların user_type='driver' ve drivers tablosunda kaydı olmalı
DRIVERS = [
    ("surucu1", "password123", 40.790000, 30.370000),
    ("surucu2", "password123", 40.740000, 30.380000),
    ("surucu3", "password123", 40.785000, 30.330000),
]

LOCATION_INTERVAL = 2
PING_INTERVAL     = 4
ACCEPT_DELAY      = 2
ARRIVAL_HOLD_SEC  = 3
STEP              = 0.002
# ----------------


def log(msg: str):
    print(f"[BOT] {msg}", flush=True)


def move_toward(clat, clon, tlat, tlon, step=STEP):
    dlat = tlat - clat
    dlon = tlon - clon
    dist = math.sqrt(dlat**2 + dlon**2)
    if dist < step:
        return tlat, tlon
    ratio = step / dist
    return clat + dlat * ratio, clon + dlon * ratio


def http_post(url: str, body: dict, token: str = None) -> tuple[bool, int, str]:
    data = json.dumps(body).encode()
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


def http_get(url: str, token: str = None) -> tuple[bool, int, str]:
    headers = {"Accept": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


async def login(username: str, password: str) -> str | None:
    """Login ol, access_token döndür."""
    url = f"{API_BASE_URL}/auth/login"
    ok, code, body = await asyncio.to_thread(
        http_post, url, {"username": username, "password": password}
    )
    if not ok:
        log(f"Login başarısız ({username}): HTTP {code} — {body[:200]}")
        return None
    try:
        data = json.loads(body)
        # Mevcut auth modülünün döndürdüğü format:
        # {"success": true, "tokens": {"access_token": "..."}}
        # veya direkt {"access_token": "..."}
        token = (
            data.get("tokens", {}).get("access_token")
            or data.get("access_token")
        )
        if token:
            log(f"Login başarılı: {username}")
            return token
        log(f"Login yanıtında token yok ({username}): {body[:200]}")
        return None
    except Exception as e:
        log(f"Login parse hatası ({username}): {e}")
        return None


async def check_active_ride(token: str) -> dict | None:
    """Başlangıçta yarım kalan ride var mı kontrol et."""
    url = f"{API_BASE_URL}/ride/driver/active"
    ok, code, body = await asyncio.to_thread(http_get, url, token)
    if not ok:
        return None
    try:
        data = json.loads(body)
        return data.get("active_ride")  # None veya ride dict
    except Exception:
        return None


async def update_ride_status(ride_id: int, status: str, token: str) -> bool:
    url = f"{API_BASE_URL}/ride/{ride_id}/status"
    ok, code, body = await asyncio.to_thread(http_post, url, {"status": status}, token)
    log(f"  → ride #{ride_id} status={status} HTTP {code} {'✅' if ok else '❌'}: {body[:200]}")
    return ok


async def cancel_ride(ride_id: int, token: str) -> bool:
    url = f"{API_BASE_URL}/ride/{ride_id}/cancel"
    ok, code, body = await asyncio.to_thread(http_post, url, {"by": "driver"}, token)
    log(f"  → ride #{ride_id} cancel HTTP {code} {'✅' if ok else '❌'}: {body[:200]}")
    return ok


async def fetch_ride_coords(ride_id: int, token: str) -> tuple[float | None, float | None]:
    url = f"{API_BASE_URL}/ride/{ride_id}"
    ok, code, body = await asyncio.to_thread(http_get, url, token)
    if not ok:
        return None, None
    try:
        data = json.loads(body)
        return data.get("dropoff_lat"), data.get("dropoff_lon")
    except Exception:
        return None, None


async def run_bot(username: str, token: str, state: dict):
    ws_url = f"{WS_BASE_URL}/ws/driver?token={urllib.parse.quote(token)}"
    lat = state["lat"]
    lon = state["lon"]
    target_lat = state.get("target_lat")
    target_lon = state.get("target_lon")
    phase = state.get("phase", "idle")
    active_ride_id = state.get("active_ride_id")
    dropoff_lat = state.get("dropoff_lat")
    dropoff_lon = state.get("dropoff_lon")

    log(f"[{username}] Bağlanılıyor: {ws_url[:60]}...")

    async with websockets.connect(ws_url) as ws:
        log(f"[{username}] Bağlantı kuruldu. Teklif bekleniyor...")

        async def send(msg: dict):
            await ws.send(json.dumps(msg))

        async def location_loop():
            nonlocal lat, lon, target_lat, target_lon, phase
            nonlocal active_ride_id, dropoff_lat, dropoff_lon

            while True:
                await asyncio.sleep(LOCATION_INTERVAL)

                if target_lat is not None:
                    lat, lon = move_toward(lat, lon, target_lat, target_lon)
                    arrived = abs(lat - target_lat) < 0.00001 and abs(lon - target_lon) < 0.00001

                    if arrived:
                        log(f"[{username}] Hedefe ulaşıldı (phase={phase})")
                        lat, lon = target_lat, target_lon
                        target_lat = target_lon = None
                        state["target_lat"] = state["target_lon"] = None

                        if phase == "driving_to_pickup" and active_ride_id:
                            await asyncio.sleep(ARRIVAL_HOLD_SEC)
                            ok = await update_ride_status(active_ride_id, "picked_up", token)
                            if ok:
                                phase = "picked_up"
                                state["phase"] = phase
                                dlat = dropoff_lat
                                dlon = dropoff_lon
                                if dlat is None:
                                    dlat, dlon = await fetch_ride_coords(active_ride_id, token)
                                    state["dropoff_lat"] = dlat
                                    state["dropoff_lon"] = dlon
                                if dlat is not None:
                                    target_lat, target_lon = dlat, dlon
                                    phase = "driving_to_dropoff"
                                    state["phase"] = phase
                                    state["target_lat"] = target_lat
                                    state["target_lon"] = target_lon
                                    log(f"[{username}] Dropoff'a yürünüyor: ({target_lat:.4f}, {target_lon:.4f})")
                            else:
                                log(f"[{username}] ⚠️ picked_up başarısız")
                                target_lat, target_lon = lat, lon
                                state["target_lat"] = target_lat
                                state["target_lon"] = target_lon

                        elif phase == "driving_to_dropoff" and active_ride_id:
                            await asyncio.sleep(ARRIVAL_HOLD_SEC)
                            rid = active_ride_id
                            ok = await update_ride_status(rid, "completed", token)
                            if ok:
                                log(f"[{username}] 🎉 Yolculuk tamamlandı!")
                                phase = "idle"
                                active_ride_id = dropoff_lat = dropoff_lon = None
                                state.update({
                                    "phase": "idle",
                                    "active_ride_id": None,
                                    "dropoff_lat": None,
                                    "dropoff_lon": None,
                                    "target_lat": None,
                                    "target_lon": None,
                                })
                            else:
                                log(f"[{username}] ⚠️ completed gönderilemedi, tekrar denenecek...")
                                target_lat, target_lon = lat, lon
                                state["target_lat"] = target_lat
                                state["target_lon"] = target_lon

                state["lat"] = lat
                state["lon"] = lon

                await send({"type": "location_update", "lat": lat, "lon": lon})
                log(f"[{username}] Konum → ({lat:.6f}, {lon:.6f})")

        async def ping_loop():
            while True:
                await asyncio.sleep(PING_INTERVAL)
                await send({"type": "ping"})

        async def message_loop():
            nonlocal target_lat, target_lon, phase, active_ride_id
            nonlocal dropoff_lat, dropoff_lon

            async for raw in ws:
                try:
                    msg = json.loads(raw)
                except json.JSONDecodeError:
                    continue

                mtype = msg.get("type")

                if mtype == "ride_offer":
                    ride_id = msg["ride_id"]
                    active_ride_id = ride_id
                    pickup_lat = msg.get("pickup_lat")
                    pickup_lon = msg.get("pickup_lon")
                    dropoff_lat = msg.get("dropoff_lat")
                    dropoff_lon = msg.get("dropoff_lon")
                    state.update({
                        "active_ride_id": active_ride_id,
                        "dropoff_lat": dropoff_lat,
                        "dropoff_lon": dropoff_lon,
                    })
                    log(f"[{username}] Teklif! ride #{ride_id} | {msg.get('pickup_address')} → {msg.get('dropoff_address')}")

                    await asyncio.sleep(ACCEPT_DELAY)
                    await send({"type": "offer_response", "ride_id": ride_id, "accepted": True})
                    log(f"[{username}] Kabul edildi ✅ ride #{ride_id}")

                    target_lat = pickup_lat
                    target_lon = pickup_lon
                    phase = "driving_to_pickup"
                    state.update({
                        "phase": phase,
                        "target_lat": target_lat,
                        "target_lon": target_lon,
                    })
                    log(f"[{username}] Pickup'a yürünüyor: ({target_lat:.4f}, {target_lon:.4f})")

                elif mtype == "ride_status_changed":
                    status = msg.get("status")
                    log(f"[{username}] Ride durumu: {status}")
                    if status in ("completed", "cancelled", "no_driver"):
                        target_lat = target_lon = None
                        phase = "idle"
                        active_ride_id = dropoff_lat = dropoff_lon = None
                        state.update({
                            "phase": "idle",
                            "active_ride_id": None,
                            "dropoff_lat": None,
                            "dropoff_lon": None,
                            "target_lat": None,
                            "target_lon": None,
                        })
                        log(f"[{username}] Müsait, yeni teklif bekleniyor...")

                elif mtype == "pong":
                    pass

                elif mtype == "error":
                    log(f"[{username}] Sunucu hatası: {msg.get('message')}")

        await asyncio.gather(location_loop(), ping_loop(), message_loop())


async def driver_loop(username: str, password: str, start_lat: float, start_lon: float):
    retry_delay = 5
    state = {
        "lat": start_lat, "lon": start_lon,
        "target_lat": None, "target_lon": None,
        "phase": "idle", "active_ride_id": None,
        "dropoff_lat": None, "dropoff_lon": None,
    }

    while True:
        # Login
        token = await login(username, password)
        if not token:
            log(f"[{username}] Login başarısız. {retry_delay}sn sonra tekrar...")
            await asyncio.sleep(retry_delay)
            continue

        # Başlangıçta yarım kalan ride kontrolü
        active = await check_active_ride(token)
        if active:
            log(f"[{username}] Yarım kalan ride bulundu: #{active['ride_id']} ({active['status']}) — completed olarak işaretleniyor")
            await update_ride_status(active["ride_id"], "completed", token)

        # WS bağlan
        try:
            await run_bot(username, token, state)
        except websockets.exceptions.ConnectionClosedError as e:
            log(f"[{username}] Bağlantı kesildi: {e}. {retry_delay}sn sonra yeniden...")
        except websockets.exceptions.InvalidStatusCode as e:
            log(f"[{username}] WS bağlantı hatası (HTTP {e.status_code}) — token süresi dolmuş olabilir. Yeniden login...")
        except OSError as e:
            log(f"[{username}] Bağlantı hatası: {e}. {retry_delay}sn sonra...")
        except Exception as e:
            log(f"[{username}] Beklenmedik hata: {e}. {retry_delay}sn sonra...")

        await asyncio.sleep(retry_delay)


async def main():
    await asyncio.gather(*[
        driver_loop(username, password, lat, lon)
        for username, password, lat, lon in DRIVERS
    ])


if __name__ == "__main__":
    def handle_exit(sig, frame):
        log("Bot durduruluyor...")
        sys.exit(0)

    signal.signal(signal.SIGINT, handle_exit)
    signal.signal(signal.SIGTERM, handle_exit)

    asyncio.run(main())
```

---

## Görev 8 — Test kullanıcıları oluştur

Bot için `user_type = 'driver'` olan kullanıcıları register endpoint'i ile oluştur:

```bash
for i in 1 2 3; do
  curl -X POST https://one.web.tr/api/auth/register \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"surucu$i\",\"password\":\"password123\",\"email\":\"surucu$i@test.com\"}"
done
```

Sonra DB'de `user_type`'ı güncelle:

```sql
UPDATE users SET user_type = 'driver'
WHERE username IN ('surucu1', 'surucu2', 'surucu3');
```

Sonra her kullanıcı için `drivers` tablosuna kayıt ekle:

```sql
INSERT INTO drivers (user_id, full_name, phone, vehicle_plate, vehicle_model, is_active)
SELECT id,
       'Bot Sürücü ' || SUBSTRING(username FROM 7),
       '0555000000' || SUBSTRING(username FROM 7),
       '34 BOT 00' || SUBSTRING(username FROM 7),
       'Toyota Corolla',
       true
FROM users
WHERE username IN ('surucu1', 'surucu2', 'surucu3')
ON CONFLICT DO NOTHING;
```

---

## Kurallar

- Backend `cargo check` geçmeli
- Flutter `flutter analyze` temiz geçmeli
- Bot için: `python3 -m py_compile fake_driver_bot.py`
- `/api/ride/driver/active` route'u `/api/ride/:id`'den önce tanımlanmalı — aksi halde 404 alırsın
- Bot `DRIVERS` listesindeki `username`/`password` değerleri DB'deki gerçek sürücü kullanıcılarıyla eşleşmeli
- `dismissOnTouchOutside: false` kaldırılmamalı — sürücü dialogu kapatamaz
- Flutter'da `_resolveStaleRide` hata durumunda döngüye girmemeli — en fazla bir kez tekrar dene
