# Ride Modülü v2 — Düzeltme ve İyileştirme Prompt

## Mevcut kod yapısı

```
backend_ride/
├── controllers/ride.rs
├── dispatch.rs
├── entities/drivers.rs, rides.rs, ride_offers.rs
├── ws/handler.rs
├── ws/hub.rs
└── ws/messages.rs

flutter_lib/modules/ride_sharing/
├── home_page.dart
├── providers/ride_provider.dart
├── services/ride_service.dart
├── services/ride_ws_service.dart
└── services/route_service.dart

fake_driver_bot.py
```

---

## BACKEND DÜZELTMELERİ

### Düzeltme 1 — DB konum yazımını tek sorguya indir

**Dosya:** `src/modules/ride/ws/handler.rs`

`LocationUpdate` handler'ında şu iki adımlı işlemi kaldır:

```rust
// KALDIR — bu iki sorgu:
if let Ok(Some(driver)) = Driver::find_by_id(driver_id).one(&state.db).await {
    let mut active: drivers::ActiveModel = driver.into();
    active.current_lat = Set(Some(lat));
    active.current_lon = Set(Some(lon));
    active.location_updated_at = Set(Some(chrono::Utc::now().into()));
    active.update(&state.db).await.ok();
}
```

Yerine tek UPDATE sorgusu kullan:

```rust
// EKLE — tek sorgu:
use sea_orm::sea_query::Expr;
drivers::Entity::update_many()
    .col_expr(drivers::Column::CurrentLat, Expr::value(lat))
    .col_expr(drivers::Column::CurrentLon, Expr::value(lon))
    .col_expr(
        drivers::Column::LocationUpdatedAt,
        Expr::value(chrono::Utc::now()),
    )
    .filter(drivers::Column::Id.eq(driver_id))
    .exec(&state.db)
    .await
    .ok();
```

Gerekli import ekle:
```rust
use sea_orm::{sea_query::Expr, ColumnTrait, QueryFilter};
```

---

### Düzeltme 2 — `accepted` durumunda da sürücü konumu yolcuya iletilsin

**Dosya:** `src/modules/ride/ws/handler.rs`

Aktif ride sorgusu zaten `Accepted` ve `PickedUp` her ikisini de kapsıyor — kontrol et, yoksa ekle:

```rust
let active_ride = Ride::find()
    .filter(rides::Column::DriverId.eq(driver_id))
    .filter(
        Condition::any()
            .add(rides::Column::Status.eq(RideStatus::Accepted))
            .add(rides::Column::Status.eq(RideStatus::PickedUp)),
    )
    .one(&state.db)
    .await
    .ok()
    .flatten();
```

`Condition` import'u yoksa ekle:
```rust
use sea_orm::Condition;
```

---

## BOT DÜZELTMELERİ

### Düzeltme 3 — Bot dropoff'a varınca `completed` göndersin

**Dosya:** `fake_driver_bot.py`

`location_loop` içindeki `driving_to_dropoff` fazını güncelle.

Mevcut kod:
```python
elif phase == "driving_to_dropoff" and active_ride_id:
    await asyncio.sleep(ARRIVAL_HOLD_SEC)
    log(f"[D{driver_id}] ✅ Varış noktasına ulaşıldı. Yolcunun tamamlaması bekleniyor...")
    phase = "arrived"
    state["phase"] = phase
```

Yeni kod:
```python
elif phase == "driving_to_dropoff" and active_ride_id:
    await asyncio.sleep(ARRIVAL_HOLD_SEC)
    log(f"[D{driver_id}] ✅ Varış noktasına ulaşıldı, yolculuk tamamlanıyor...")
    ok = await update_ride_status(active_ride_id, "completed")
    if ok:
        log(f"[D{driver_id}] 🎉 Yolculuk tamamlandı!")
        phase = "idle"
        active_ride_id = None
        dropoff_lat = None
        dropoff_lon = None
        state.update({
            "phase": "idle",
            "active_ride_id": None,
            "dropoff_lat": None,
            "dropoff_lon": None,
            "target_lat": None,
            "target_lon": None,
        })
    else:
        log(f"[D{driver_id}] ⚠️ completed gönderilemedi, tekrar denenecek...")
        # Aynı noktada bekle, bir sonraki location_loop tick'inde tekrar dene
        target_lat = lat
        target_lon = lon
        state["target_lat"] = target_lat
        state["target_lon"] = target_lon
```

---

## FLUTTER DÜZELTMELERİ

### Düzeltme 4 — Race condition: WS bağlandıktan sonra ride isteği gönder

**Dosya:** `lib/modules/ride_sharing/providers/ride_provider.dart`

`requestRide()` metodunda WS bağlantısının tam kurulmasını bekle. Mevcut `connect()` metodu fire-and-forget çalışıyor. `RideWsService.connect()` içine kısa bir bağlantı bekleme ekle:

**`ride_ws_service.dart`** — `connect()` metodunu güncelle:

```dart
Future<void> connect(int userId) async {
  disconnect();

  _messageController = StreamController<ServerMessage>.broadcast();

  final uri = Uri.parse('${AppConfig.wsBaseUrl}/ws/passenger?user_id=$userId');
  debugPrint('RideWsService: connecting to $uri');

  _channel = WebSocketChannel.connect(uri);

  // Bağlantının kurulmasını bekle
  try {
    await _channel!.ready;
    debugPrint('RideWsService: connection ready');
  } catch (e) {
    debugPrint('RideWsService: connection failed: $e');
  }

  _channel!.stream.listen(
    (data) {
      debugPrint('RideWsService: received: $data');
      final msg = _parseMessage(data as String);
      _messageController?.add(msg);
    },
    onError: (error) {
      debugPrint('RideWsService: error: $error');
      _messageController?.add(ErrorMessage(error.toString()));
    },
    onDone: () {
      debugPrint('RideWsService: connection closed');
    },
  );

  startPingTimer();
}
```

**`ride_provider.dart`** — `requestRide()` içinde WS sub'ı bağlantıdan hemen sonra kur:

```dart
Future<void> requestRide() async {
  final current = state.currentLocation;
  final dest = state.destination;
  final destAddress = state.destinationAddress ?? '';

  if (current == null || dest == null) return;

  state = state.copyWith(rideStatus: 'searching');

  // 1. WS bağlantısını aç ve dinlemeye başla
  await _connectWs();

  // 2. Kısa bekleme — stream listener'ın kurulması için
  await Future.delayed(const Duration(milliseconds: 100));

  // 3. Artık backend'e istek gönder
  final response = await RideService.requestRide(
    pickupLat: current.latitude,
    pickupLon: current.longitude,
    pickupAddress: 'Mevcut Konum',
    dropoffLat: dest.latitude,
    dropoffLon: dest.longitude,
    dropoffAddress: destAddress,
  );

  if (!response.success || response.rideId == null) {
    debugPrint('RideNotifier: requestRide failed: ${response.error}');
    _wsSub?.cancel();
    _wsService.disconnect();
    state = state.copyWith(rideStatus: 'idle');
    return;
  }

  final rideId = response.rideId!;
  state = state.copyWith(
    activeRideId: rideId,
    rideStatus: response.status ?? 'searching',
  );

  _startPolling(rideId);
}
```

---

### Düzeltme 5 — `accepted` durumunda da harita sürücüyü takip etsin

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`ref.listen` bloğunu güncelle:

Mevcut:
```dart
ref.listen<RideState>(rideProvider, (prev, next) {
  if (next.rideStatus == 'picked_up' && prev?.rideStatus != 'picked_up') {
    _cameraFollowing = true;
  }
  if (next.driverLocation != null &&
      next.driverLocation != prev?.driverLocation &&
      _cameraFollowing &&
      next.rideStatus == 'picked_up') {
    _mapController.move(next.driverLocation!, _mapController.camera.zoom);
  }
});
```

Yeni:
```dart
ref.listen<RideState>(rideProvider, (prev, next) {
  // accepted veya picked_up olunca kamera takibini aç
  if ((next.rideStatus == 'accepted' || next.rideStatus == 'picked_up') &&
      prev?.rideStatus != 'accepted' && prev?.rideStatus != 'picked_up') {
    _cameraFollowing = true;
  }
  // Sürücü konumu değişince haritayı kaydır
  if (next.driverLocation != null &&
      next.driverLocation != prev?.driverLocation &&
      _cameraFollowing &&
      (next.rideStatus == 'accepted' || next.rideStatus == 'picked_up')) {
    _mapController.move(next.driverLocation!, _mapController.camera.zoom);
  }
});
```

---

### Düzeltme 6 — ETA `etaSeconds` öncelikli kullanılsın

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`_buildRideStatusCard` içindeki `accepted` case'inde `etaText` hesaplamasını güncelle:

Mevcut:
```dart
String? etaText;
if (rideState.routeInfo != null) {
  etaText = rideState.routeInfo!.formattedDuration;
} else if (rideState.etaSeconds != null) {
  final mins = (rideState.etaSeconds! / 60).ceil();
  etaText = '$mins dk';
}
```

Yeni — `etaSeconds` önce gelsin (sürücü→yolcu süresi), `routeInfo` yolcu→varış rotası:
```dart
String? etaText;
if (rideState.etaSeconds != null) {
  // Backend'den gelen sürücü ETA'sı
  final mins = (rideState.etaSeconds! / 60).ceil();
  etaText = '~$mins dk';
} else if (rideState.routeInfo != null) {
  etaText = rideState.routeInfo!.formattedDuration;
}
```

---

### Düzeltme 7 — `completed` kartında ücret gösterilsin

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`completed` case'ini güncelle:

Mevcut:
```dart
case 'completed':
  return Column(
    mainAxisSize: MainAxisSize.min,
    children: [
      Row(
        children: [
          const Icon(Icons.check_circle, color: Colors.green),
          const SizedBox(width: 12),
          const Expanded(
            child: Text(
              'Yolculuk tamamlandı',
              style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16),
            ),
          ),
          TextButton(
            onPressed: () => ref.read(rideProvider.notifier).resetRide(),
            child: const Text('Kapat'),
          ),
        ],
      ),
      const SizedBox(height: 4),
      const Text(
        'Ödemeyi sürücüye nakit veya kart ile yapabilirsiniz.',
        style: TextStyle(color: Colors.grey, fontSize: 13),
      ),
    ],
  );
```

Yeni:
```dart
case 'completed':
  final fare = rideState.fareAmount;
  return Column(
    mainAxisSize: MainAxisSize.min,
    children: [
      Row(
        children: [
          const Icon(Icons.check_circle, color: Colors.green, size: 28),
          const SizedBox(width: 12),
          const Expanded(
            child: Text(
              'Yolculuk tamamlandı',
              style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16),
            ),
          ),
          TextButton(
            onPressed: () {
              ref.read(rideProvider.notifier).resetRide();
              _destinationController.clear();
            },
            child: const Text('Kapat'),
          ),
        ],
      ),
      const SizedBox(height: 8),
      Container(
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: Colors.green.withAlpha(20),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            const Text(
              'Ödeme tutarı',
              style: TextStyle(color: Colors.grey, fontSize: 14),
            ),
            Text(
              fare != null && fare > 0
                  ? '₺${fare.toStringAsFixed(2)}'
                  : 'Nakit veya kart',
              style: const TextStyle(
                fontWeight: FontWeight.bold,
                fontSize: 16,
                color: Colors.green,
              ),
            ),
          ],
        ),
      ),
      const SizedBox(height: 8),
      const Text(
        'Ödemeyi sürücüye yapabilirsiniz.',
        style: TextStyle(color: Colors.grey, fontSize: 13),
      ),
    ],
  );
```

---

### Düzeltme 8 — `no_driver` → "Tekrar Dene" destination'ı korusun

**Dosya:** `lib/modules/ride_sharing/providers/ride_provider.dart`

`resetRide()` metodunu güncelle — destination ve route'u koru:

Mevcut:
```dart
void resetRide() {
  _stopPolling();
  _wsSub?.cancel();
  _wsService.disconnect();
  state = RideState(
    currentLocation: state.currentLocation,
  );
}
```

Yeni:
```dart
void resetRide({bool keepDestination = false}) {
  _stopPolling();
  _wsSub?.cancel();
  _wsService.disconnect();
  if (keepDestination) {
    state = RideState(
      currentLocation: state.currentLocation,
      destination: state.destination,
      destinationAddress: state.destinationAddress,
      routePoints: state.routePoints,
      routeInfo: state.routeInfo,
    );
  } else {
    state = RideState(currentLocation: state.currentLocation);
  }
}
```

**`home_page.dart`** — `no_driver` case'inde "Tekrar Dene" butonunu güncelle:

```dart
TextButton(
  onPressed: () async {
    // Destination'ı koruyarak sıfırla
    ref.read(rideProvider.notifier).resetRide(keepDestination: true);
    await Future.delayed(const Duration(milliseconds: 300));
    await ref.read(rideProvider.notifier).requestRide();
  },
  style: TextButton.styleFrom(foregroundColor: Colors.green),
  child: const Text('Tekrar Dene'),
),
```

"Kapat" butonu destination'ı temizlesin:
```dart
TextButton(
  onPressed: () {
    ref.read(rideProvider.notifier).resetRide();
    _destinationController.clear();
  },
  child: const Text('Kapat'),
),
```

---

### Düzeltme 9 — `_PulsingMarker` üzerine taksi ikonu ekle

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`_PulsingMarkerState.build()` metodunu güncelle:

```dart
@override
Widget build(BuildContext context) {
  return Stack(
    alignment: Alignment.center,
    children: [
      // Dalgalanan halka
      AnimatedBuilder(
        animation: _ctrl,
        builder: (_, __) => Transform.scale(
          scale: _scale.value,
          child: Container(
            width: 48,
            height: 48,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: Colors.green.withAlpha((_opacity.value * 255).toInt()),
            ),
          ),
        ),
      ),
      // Sabit merkez — taksi ikonu
      Container(
        width: 24,
        height: 24,
        decoration: const BoxDecoration(
          shape: BoxShape.circle,
          color: Colors.green,
        ),
        child: const Icon(
          Icons.local_taxi,
          color: Colors.white,
          size: 16,
        ),
      ),
    ],
  );
}
```

---

## Kurallar

- Backend için `cargo check` geçmeli
- Flutter için `flutter analyze` temiz geçmeli
- Mevcut dosyalara yalnızca belirtilen satırları değiştir
- `sea_orm::sea_query::Expr` import'unu `handler.rs`'e ekle
- Bot syntax kontrolü: `python3 -m py_compile fake_driver_bot.py`
- Değişiklik sonrası bot yeniden başlatılmalı
