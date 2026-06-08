# Ride Modülü — Hata Düzeltme ve UX İyileştirme Prompt

## Mevcut kod yapısı

```
rust_ride/
├── controllers/ride.rs     — HTTP endpointler
├── dispatch.rs             — sürücü eşleştirme loop'u
├── ws/handler.rs           — WebSocket bağlantı yönetimi
├── ws/messages.rs          — mesaj tipleri (ServerMessage, ClientMessage)
└── ws/hub.rs               — bağlantı map'leri

flutter_lib/modules/ride_sharing/
├── home_page.dart          — harita UI
├── providers/ride_provider.dart
├── services/ride_service.dart
├── services/ride_ws_service.dart
└── services/route_service.dart
```

---

## BACKEND DÜZELTMELERİ

### Düzeltme 1 — `picked_up` durumunda sürücü konumu yolcuya gitmiyor

**Dosya:** `src/modules/ride/ws/handler.rs`

`handle_driver_socket` içinde `LocationUpdate` işlendiğinde aktif ride sorgusu yalnızca `Accepted` durumu için filtreliyor. `PickedUp` durumu da eklenmeli.

Mevcut kod:
```rust
let active_ride = Ride::find()
    .filter(rides::Column::DriverId.eq(driver_id))
    .filter(rides::Column::Status.eq(RideStatus::Accepted))
    .one(&state.db)
    .await
    .ok()
    .flatten();
```

Düzeltilmiş kod:
```rust
use sea_orm::Condition;

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

---

### Düzeltme 2 — Her konum güncellemesinde DB yazımı kaldırılmalı

**Dosya:** `src/modules/ride/ws/handler.rs`

`handle_driver_socket` içinde `LocationUpdate` geldiğinde DB'ye anlık yazım yapılıyor. Bu her 2-3 saniyede bir DB yazısı demek. Location flush task zaten var, bu blok kaldırılmalı.

Şu bloğu **sil:**
```rust
// DB'ye anlık yaz
if let Ok(Some(driver)) = Driver::find_by_id(driver_id).one(&state.db).await {
    let mut active: drivers::ActiveModel = driver.into();
    active.current_lat = Set(Some(lat));
    active.current_lon = Set(Some(lon));
    active.location_updated_at = Set(Some(chrono::Utc::now().into()));
    active.update(&state.db).await.ok();
}
```

Yalnızca hub ve Redis güncellemesi kalsın:
```rust
Ok(ClientMessage::LocationUpdate { lat, lon }) => {
    // Hub cache güncelle
    if let Some(mut s) = state.hub.drivers.get_mut(&driver_id) {
        s.lat = Some(lat);
        s.lon = Some(lon);
    }

    // Redis'e yaz
    let key = format!("driver:{}:location", driver_id);
    let val = format!("{},{}", lat, lon);
    let mut redis = (*state.redis).clone();
    redis.set::<_, _, ()>(&key, &val).await.ok();

    // Aktif ride varsa yolcuya ilet (Accepted veya PickedUp)
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

    if let Some(ride) = active_ride {
        state.hub.broadcast_driver_location(ride.id, lat, lon);
    }
}
```

---

### Düzeltme 3 — `get_ride` endpoint'ine `driver_lat/lon` eklenmeli

**Dosya:** `src/modules/ride/controllers/ride.rs`

`RideDetail` struct'ına `driver_lat` ve `driver_lon` ekle — Flutter polling'den anlık konumu alabilsin:

```rust
#[derive(Debug, Serialize)]
struct RideDetail {
    id: i64,
    user_id: i64,
    status: String,
    pickup_address: String,
    dropoff_address: String,
    distance_km: Option<f64>,
    duration_sec: Option<i32>,
    fare_amount: Option<f64>,  // buraya da ekle
    driver: Option<DriverInfo>,
}
```

`get_ride` handler'ında `fare_amount`'u da response'a ekle:
```rust
let detail = RideDetail {
    id: ride.id,
    user_id: ride.user_id,
    status: ride.status.as_str().to_string(),
    pickup_address: ride.pickup_address,
    dropoff_address: ride.dropoff_address,
    distance_km: ride.distance_km,
    duration_sec: ride.duration_sec,
    fare_amount: ride.fare_amount
        .as_ref()
        .and_then(|d| d.to_string().parse::<f64>().ok()),
    driver: driver_info,
};
```

---

### Düzeltme 4 — `cancel_ride` sonrası ride_rooms temizlenmeli

**Dosya:** `src/modules/ride/controllers/ride.rs`

`cancel_ride` handler'ında, bildirim gönderdikten sonra hub'dan ride_room kaydını sil:

```rust
// Mevcut bildirim kodundan sonra ekle:
state.hub.ride_rooms.remove(&id);
```

Aynı şekilde `update_ride_status` handler'ında `completed` durumuna geçince de ekle:
```rust
if new_status == RideStatus::Completed {
    state.hub.ride_rooms.remove(&id);
}
```

---

## FLUTTER DÜZELTMELERİ

### Düzeltme 5 — `accepted` durumunda da kamera sürücüyü takip etsin

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`ref.listen` bloğunu güncelle — sadece `picked_up` değil `accepted` durumunda da sürücüyü takip et:

Mevcut:
```dart
ref.listen<RideState>(rideProvider, (prev, next) {
  if (next.driverLocation != null &&
      next.driverLocation != prev?.driverLocation &&
      _cameraFollowing &&
      next.rideStatus == 'picked_up') {
    _mapController.move(next.driverLocation!, _mapController.camera.zoom);
  }
});
```

Düzeltilmiş:
```dart
ref.listen<RideState>(rideProvider, (prev, next) {
  if (next.driverLocation != null &&
      next.driverLocation != prev?.driverLocation &&
      _cameraFollowing &&
      (next.rideStatus == 'accepted' || next.rideStatus == 'picked_up')) {
    _mapController.move(next.driverLocation!, _mapController.camera.zoom);
  }
});
```

---

### Düzeltme 6 — Sürücü bilgi kartına telefon ve ETA ekle

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`_buildRideStatusCard` içindeki `accepted` ve `picked_up` case'lerini güncelle. Her ikisine de telefon arama butonu ve ETA ekle:

`accepted` case'i:
```dart
case 'accepted':
  final driver = rideState.assignedDriver;
  final eta = rideState.routeInfo?.formattedDuration;
  return Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    mainAxisSize: MainAxisSize.min,
    children: [
      Row(
        children: [
          const Icon(Icons.local_taxi, color: Colors.amber, size: 28),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  driver?.fullName ?? 'Sürücü',
                  style: const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                ),
                Text(
                  '${driver?.vehicleModel ?? ''} • ${driver?.vehiclePlate ?? ''}',
                  style: TextStyle(color: Colors.grey[600], fontSize: 13),
                ),
              ],
            ),
          ),
          // Telefon butonu
          if (driver?.phone != null)
            IconButton(
              icon: const Icon(Icons.phone, color: Colors.green),
              onPressed: () async {
                final uri = Uri.parse('tel:${driver!.phone}');
                if (await canLaunchUrl(uri)) launchUrl(uri);
              },
            ),
        ],
      ),
      const SizedBox(height: 4),
      Row(
        children: [
          const Icon(Icons.directions_car, size: 16, color: Colors.green),
          const SizedBox(width: 4),
          Text(
            'Sürücü yolda',
            style: const TextStyle(color: Colors.green, fontWeight: FontWeight.w500),
          ),
          if (eta != null) ...[
            const SizedBox(width: 8),
            Text(
              '• $eta',
              style: TextStyle(color: Colors.grey[600], fontSize: 13),
            ),
          ],
        ],
      ),
      const SizedBox(height: 12),
      SizedBox(
        width: double.infinity,
        child: OutlinedButton.icon(
          onPressed: () => _cancelRide(),
          icon: const Icon(Icons.cancel_outlined, size: 18),
          label: const Text('Yolculuğu İptal Et'),
          style: OutlinedButton.styleFrom(
            foregroundColor: Colors.red,
            side: const BorderSide(color: Colors.red),
          ),
        ),
      ),
    ],
  );
```

`picked_up` case'i:
```dart
case 'picked_up':
  final driver = rideState.assignedDriver;
  return Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    mainAxisSize: MainAxisSize.min,
    children: [
      Row(
        children: [
          const Icon(Icons.local_taxi, color: Colors.amber, size: 28),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  driver?.fullName ?? 'Sürücü',
                  style: const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                ),
                Text(
                  '${driver?.vehicleModel ?? ''} • ${driver?.vehiclePlate ?? ''}',
                  style: TextStyle(color: Colors.grey[600], fontSize: 13),
                ),
              ],
            ),
          ),
          if (driver?.phone != null)
            IconButton(
              icon: const Icon(Icons.phone, color: Colors.green),
              onPressed: () async {
                final uri = Uri.parse('tel:${driver!.phone}');
                if (await canLaunchUrl(uri)) launchUrl(uri);
              },
            ),
        ],
      ),
      const SizedBox(height: 4),
      Row(
        children: [
          const Icon(Icons.check_circle, size: 16, color: Colors.green),
          const SizedBox(width: 4),
          const Text(
            'Yolculuk başladı',
            style: TextStyle(color: Colors.green, fontWeight: FontWeight.w500),
          ),
        ],
      ),
      const SizedBox(height: 12),
      SizedBox(
        width: double.infinity,
        child: ElevatedButton.icon(
          onPressed: () => ref.read(rideProvider.notifier).completeRide(),
          icon: const Icon(Icons.check_circle, size: 18),
          label: const Text('Yolculuğu Tamamla'),
          style: ElevatedButton.styleFrom(
            backgroundColor: Colors.green,
            foregroundColor: Colors.white,
          ),
        ),
      ),
    ],
  );
```

`pubspec.yaml`'a ekle (yoksa):
```yaml
url_launcher: ^6.2.0
```

`home_page.dart`'a import ekle:
```dart
import 'package:url_launcher/url_launcher.dart';
```

---

### Düzeltme 7 — `completed` sonrası rota ve destination temizlensin

**Dosya:** `lib/modules/ride_sharing/providers/ride_provider.dart`

`resetRide()` metodunu güncelle — destination ve route bilgilerini de temizle:

```dart
void resetRide() {
  _stopPolling();
  stopFakeDriverMovement();
  _wsSub?.cancel();
  _wsService.disconnect();
  // Tüm state sıfırla, sadece mevcut konumu koru
  state = RideState(
    currentLocation: state.currentLocation,
  );
}
```

---

### Düzeltme 8 — `no_driver` durumunda "Tekrar Dene" önce cleanup yapsın

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`no_driver` case'indeki "Tekrar Dene" butonunu güncelle:

```dart
case 'no_driver':
  return Row(
    children: [
      const Icon(Icons.warning_amber, color: Colors.orange),
      const SizedBox(width: 12),
      const Expanded(child: Text('Yakında sürücü bulunamadı')),
      TextButton(
        onPressed: () => ref.read(rideProvider.notifier).resetRide(),
        child: const Text('Kapat'),
      ),
      TextButton(
        onPressed: () async {
          // Önce eski bağlantıyı temizle, sonra yeni istek gönder
          ref.read(rideProvider.notifier).resetRide();
          await Future.delayed(const Duration(milliseconds: 300));
          await ref.read(rideProvider.notifier).requestRide();
        },
        style: TextButton.styleFrom(foregroundColor: Colors.green),
        child: const Text('Tekrar Dene'),
      ),
    ],
  );
```

---

### Düzeltme 9 — ETA backend'den alınan `duration_sec` ile gösterilsin

**Dosya:** `lib/modules/ride_sharing/providers/ride_provider.dart`

`RideState`'e `etaSeconds` alanı ekle:

```dart
final int? etaSeconds;

// constructor'a ekle
RideState({
  ...
  this.etaSeconds,
});

// copyWith'e ekle
Object? etaSeconds = _sentinel,
// ...
etaSeconds: etaSeconds == _sentinel ? this.etaSeconds : etaSeconds as int?,
```

`_startPolling` içinde `durationSec` gelince state'e yaz:
```dart
state = state.copyWith(
  rideStatus: response.status,
  assignedDriver: response.driver,
  fareAmount: response.fareAmount,
  etaSeconds: response.durationSec,
  // ...
);
```

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`accepted` kartında ETA'yı `rideState.etaSeconds` üzerinden hesapla:
```dart
// routeInfo yoksa etaSeconds'dan hesapla
String? etaText;
if (rideState.routeInfo != null) {
  etaText = rideState.routeInfo!.formattedDuration;
} else if (rideState.etaSeconds != null) {
  final mins = (rideState.etaSeconds! / 60).ceil();
  etaText = '$mins dk';
}
```

---

## Kurallar

- Backend için `cargo check` geçmeli
- Flutter için `flutter analyze` temiz geçmeli
- Mevcut dosyalara belirtilmeyen yerlere dokunma
- `url_launcher` paketi eklenince `flutter pub get` çalıştır
- Import'lar temiz olsun, kullanılmayan import ekleme
- Rust tarafında `use sea_orm::Condition;` import'unu ilgili dosyaya ekle
