# Flutter Ride UI Düzeltmeleri — Agent Prompt

## Mevcut durum

- Taksi çağırma akışı çalışıyor
- Sürücü kabul edince "Sürücü yolda" mesajı ekranda görünüyor
- Ancak üç sorun var:
  1. "Taksi Çağır" butonu ride aktifken hala tıklanabilir
  2. Sürücü haritada görünmüyor (driverLocation state güncellenmiyor)
  3. Sürücü konumu gelince harita kaymıyor

---

## Görev 1 — "Taksi Çağır" butonu devre dışı bırakma

`lib/modules/ride_sharing/home_page.dart` içindeki "Taksi Çağır" butonunun `onPressed`'ini güncelle:

```dart
ElevatedButton(
  onPressed: rideState.rideStatus == 'idle'
      ? () async {
          final notifier = ref.read(rideProvider.notifier);
          await notifier.requestRide();
        }
      : null, // aktif ride varsa buton disabled
  style: ElevatedButton.styleFrom(
    backgroundColor: Colors.green,
    foregroundColor: Colors.white,
    disabledBackgroundColor: Colors.grey,
    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(8),
    ),
  ),
  child: Text(
    rideState.rideStatus == 'idle' ? 'Taksi Çağır' : 'Bekleniyor...',
  ),
),
```

---

## Görev 2 — Sürücü marker'ı haritada göster

### 2a. RideNotifier'da DriverLocationMessage handler'ını kontrol et

`lib/modules/ride_sharing/providers/ride_provider.dart` içinde `_handleWsMessage` metodunu bul. `DriverLocationMessage` gelince state'in güncellendiğinden emin ol:

```dart
void _handleWsMessage(ServerMessage msg) {
  switch (msg) {
    case DriverLocationMessage m:
      state = state.copyWith(
        driverLocation: LatLng(m.lat, m.lon),
      );
    case RideStatusChangedMessage m:
      state = state.copyWith(rideStatus: m.status);
      if (m.status == 'completed' || m.status == 'no_driver') {
        _stopPolling();
        disconnect();
      }
    case OfferExpiredMessage _:
      // gerekirse UI'a yansıt
      break;
    case PongMessage _:
      break;
    case ErrorMessage m:
      debugPrint('WS Error: ${m.message}');
  }
}
```

### 2b. Harita children listesine sürücü marker'ı ekle

`lib/modules/ride_sharing/home_page.dart` içindeki `FlutterMap` widget'ının `children` listesine, diğer `MarkerLayer`'lardan sonra ekle:

```dart
// Sürücü konumu (sarı taksi ikonu)
if (rideState.driverLocation != null)
  MarkerLayer(
    markers: [
      Marker(
        point: rideState.driverLocation!,
        width: 48,
        height: 48,
        child: const Icon(
          Icons.local_taxi,
          color: Colors.amber,
          size: 40,
        ),
      ),
    ],
  ),
```

---

## Görev 3 — Sürücü konumu gelince haritayı kaydır

`lib/modules/ride_sharing/home_page.dart` içindeki `build` metoduna, mevcut `ref.watch` satırlarından hemen sonra `ref.listen` ekle:

```dart
// Sürücü konumu değişince haritayı kaydır
ref.listen<RideState>(rideProvider, (prev, next) {
  if (next.driverLocation != null &&
      next.driverLocation != prev?.driverLocation) {
    _mapController.move(next.driverLocation!, 14);
  }
});
```

---

## Görev 4 — Fake sürücü hareketi (test modu)

`lib/modules/ride_sharing/providers/ride_provider.dart` içine test için sahte sürücü hareketi simülasyonu ekle. Bu sadece debug modda çalışmalı.

`RideNotifier`'a şu metodu ekle:

```dart
Timer? _fakeDriverTimer;

// Test için: sürücü konumunu kademeli olarak pickup noktasına doğru hareket ettir
void startFakeDriverMovement() {
  if (!AppConfig.isDebugMode) return;

  // Başlangıç noktası: Sakarya Serdivan yakını (backend'e kaydettiğin test koordinatı)
  double lat = 40.772411;
  double lon = 30.363073;

  // Hedef: pickup noktası (state'den al)
  final targetLat = state.currentLocation?.latitude ?? 40.7604062;
  final targetLon = state.currentLocation?.longitude ?? 30.3629614;

  int steps = 0;
  const totalSteps = 20;

  _fakeDriverTimer = Timer.periodic(const Duration(seconds: 2), (timer) {
    if (steps >= totalSteps) {
      timer.cancel();
      return;
    }

    // Her adımda hedefe doğru biraz ilerle
    final progress = steps / totalSteps;
    final currentLat = lat + (targetLat - lat) * progress;
    final currentLon = lon + (targetLon - lon) * progress;

    state = state.copyWith(
      driverLocation: LatLng(currentLat, currentLon),
    );

    steps++;
  });
}

void stopFakeDriverMovement() {
  _fakeDriverTimer?.cancel();
  _fakeDriverTimer = null;
}
```

`requestRide()` metodunun başarılı olup `rideStatus == 'accepted'` durumuna geçtiği yerde (veya `RideStatusChangedMessage` handler'ında `accepted` gelince) `startFakeDriverMovement()` çağır:

```dart
case RideStatusChangedMessage m:
  state = state.copyWith(rideStatus: m.status);
  if (m.status == 'accepted') {
    startFakeDriverMovement(); // test için
  }
  if (m.status == 'completed' || m.status == 'no_driver') {
    stopFakeDriverMovement();
    _stopPolling();
  }
```

`dispose` veya `resetRide` metodunda da `stopFakeDriverMovement()` çağır.

---

## Görev 5 — Ride durum kartı iyileştirmesi

`lib/modules/ride_sharing/home_page.dart` içinde haritanın altına durum kartı ekle. Mevcut snackbar'ı kaldır, yerine kalıcı bir `Positioned` kart koy:

```dart
// Stack içine, en alta ekle
if (rideState.rideStatus != 'idle')
  Positioned(
    bottom: 0,
    left: 0,
    right: 0,
    child: Container(
      padding: const EdgeInsets.fromLTRB(20, 20, 20, 32),
      decoration: const BoxDecoration(
        color: Colors.white,
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
        boxShadow: [
          BoxShadow(
            color: Colors.black26,
            blurRadius: 12,
            offset: Offset(0, -2),
          ),
        ],
      ),
      child: _buildRideStatusCard(rideState),
    ),
  ),
```

`_buildRideStatusCard` metodunu ekle:

```dart
Widget _buildRideStatusCard(RideState rideState) {
  switch (rideState.rideStatus) {
    case 'searching':
      return const Row(
        children: [
          CircularProgressIndicator(strokeWidth: 2),
          SizedBox(width: 16),
          Text('Sürücü aranıyor...', style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold)),
        ],
      );

    case 'accepted':
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
              Column(
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
            ],
          ),
          const SizedBox(height: 8),
          Text(
            rideState.rideStatus == 'accepted' ? 'Sürücü yolda' : 'Yolculuk başladı',
            style: const TextStyle(color: Colors.green, fontWeight: FontWeight.w500),
          ),
        ],
      );

    case 'no_driver':
      return Row(
        children: [
          const Icon(Icons.warning_amber, color: Colors.orange),
          const SizedBox(width: 12),
          const Expanded(child: Text('Yakında sürücü bulunamadı')),
          TextButton(
            onPressed: () => ref.read(rideProvider.notifier).resetRide(),
            child: const Text('Tekrar Dene'),
          ),
        ],
      );

    case 'completed':
      return Row(
        children: [
          const Icon(Icons.check_circle, color: Colors.green),
          const SizedBox(width: 12),
          Text(
            'Yolculuk tamamlandı'
            '${rideState.fareAmount != null ? ' • ${rideState.fareAmount!.toStringAsFixed(2)} TL' : ''}',
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
          const Spacer(),
          TextButton(
            onPressed: () => ref.read(rideProvider.notifier).resetRide(),
            child: const Text('Kapat'),
          ),
        ],
      );

    default:
      return const SizedBox.shrink();
  }
}
```

---

## Kurallar

- `AppConfig.isDebugMode` zaten `true` — fake hareket sadece debug modda çalışır
- Mevcut `RouteService`, `GeocodingService`, `AuthService` dosyalarına dokunma
- `flutter analyze` temiz geçmeli
- Kullanılmayan import'lar ekleme
