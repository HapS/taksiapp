# Rota Backend Entegrasyonu + Sürücü UI İyileştirmeleri — Agent Prompt

## Mevcut kod yapısı

```
flutter_lib/modules/ride_sharing/
├── home_page.dart              — yolcu harita ekranı
├── driver_home_page.dart       — sürücü harita ekranı
├── providers/ride_provider.dart
├── services/route_service.dart — ORS'ye doğrudan istek atar (değişecek)
└── services/ride_service.dart

rust_ride/controllers/ride.rs   — GET /api/ride/route endpoint'i mevcut
```

### Önemli bilgiler
- Backend `GET /api/ride/route?start_lat=&start_lon=&end_lat=&end_lon=` → `{success: true, data: [{lat, lon}]}` döner
- `AppConfig.apiEndpoint = 'https://one.web.tr/api'`
- `AuthService().getAccessToken()` → JWT token döner
- Sürücü ekranında `_fetchRoute()` zaten backend'i çağırıyor, sadece genişletilecek
- Yolcu ekranında `RouteService.getRoute()` hala doğrudan ORS'ye gidiyor — bu değişecek
- `pubspec.yaml`'a yeni paket eklemek gerekiyor: `awesome_dialog` veya `flutter_animated_dialog`

---

## GÖREV 1 — Yolcu: Rota hesaplamayı backend'e taşı

**Dosya:** `lib/modules/ride_sharing/services/route_service.dart`

`getRoute()` metodunu güncelle. ORS'ye doğrudan gitmek yerine backend proxy'yi çağır:

```dart
/// İki nokta arasında araç rotası hesaplar.
/// Backend: GET /api/ride/route?start_lat=&start_lon=&end_lat=&end_lon=
static Future<RouteInfo> getRoute(LatLng start, LatLng end) async {
  final url = Uri.parse(
    '${AppConfig.apiEndpoint}/ride/route'
    '?start_lat=${start.latitude}'
    '&start_lon=${start.longitude}'
    '&end_lat=${end.latitude}'
    '&end_lon=${end.longitude}',
  );

  try {
    // JWT token al
    final authService = AuthService();
    final token = await authService.getAccessToken();

    final response = await http.get(
      url,
      headers: {
        'Accept': 'application/json',
        if (token != null) 'Authorization': 'Bearer $token',
      },
    );

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body) as Map<String, dynamic>;
      final points = data['data'] as List?;

      if (points != null && points.length >= 2) {
        final latLngPoints = points
            .map((p) => LatLng(
                  (p as Map)['lat'] as double,
                  p['lon'] as double,
                ))
            .toList();

        // Mesafe ve süre için backend'e ikinci bir istek atmak yerine
        // başlangıç ve bitiş noktaları arasındaki Haversine mesafesini hesapla
        // (backend segments bilgisi dönmüyor, yeterince yakın tahmin)
        final distanceKm = _haversineKm(start, end);
        final durationSec = (distanceKm / 30 * 3600).round(); // 30km/h ortalama

        return RouteInfo(
          points: latLngPoints,
          distanceKm: distanceKm,
          durationSeconds: durationSec,
        );
      }
    }

    // Fallback: düz çizgi
    return RouteInfo(
      points: [start, end],
      distanceKm: _haversineKm(start, end),
      durationSeconds: (_haversineKm(start, end) / 30 * 3600).round(),
    );
  } catch (e) {
    debugPrint('RouteService.getRoute error: $e');
    return RouteInfo(
      points: [start, end],
      distanceKm: _haversineKm(start, end),
      durationSeconds: (_haversineKm(start, end) / 30 * 3600).round(),
    );
  }
}

/// Haversine formülü ile iki nokta arası mesafe (km)
static double _haversineKm(LatLng a, LatLng b) {
  const r = 6371.0;
  final dLat = _toRad(b.latitude - a.latitude);
  final dLon = _toRad(b.longitude - a.longitude);
  final h = math.sin(dLat / 2) * math.sin(dLat / 2) +
      math.cos(_toRad(a.latitude)) *
          math.cos(_toRad(b.latitude)) *
          math.sin(dLon / 2) *
          math.sin(dLon / 2);
  return r * 2 * math.atan2(math.sqrt(h), math.sqrt(1 - h));
}

static double _toRad(double deg) => deg * math.pi / 180;
```

`route_service.dart` dosyasına import ekle:
```dart
import 'dart:math' as math;
import 'package:http/http.dart' as http;
import 'dart:convert';
import '../auth/services/auth_service.dart';
```

> **NOT:** `searchAddress()` metoduna dokunma — ORS autocomplete kalabilir.

---

## GÖREV 2 — Backend: `get_route` endpoint'ine mesafe ve süre ekle

**Dosya:** `src/modules/ride/controllers/ride.rs`

`get_route` handler'ının response'una `distance_km` ve `duration_sec` ekle:

```rust
// Mevcut RoutePoint struct'ına dokunma
// Response JSON'unu güncelle:

// Koordinatları parse ettikten sonra segment bilgilerini de al:
if let Some(segments) = parsed["features"][0]["properties"]["segments"].as_array() {
    if let Some(seg) = segments.first() {
        let distance_m = seg["distance"].as_f64().unwrap_or(0.0);
        let duration_s = seg["duration"].as_f64().unwrap_or(0.0);

        return (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "data": points,
            "distance_km": distance_m / 1000.0,
            "duration_sec": duration_s as i32,
        }))).into_response();
    }
}
```

Flutter tarafında bu değerleri kullan — `getRoute()` içinde backend'den gelen `distance_km` ve `duration_sec`'i Haversine yerine kullan:

```dart
final distanceKm = (data['distance_km'] as num?)?.toDouble()
    ?? _haversineKm(start, end);
final durationSec = (data['duration_sec'] as num?)?.toInt()
    ?? (_haversineKm(start, end) / 30 * 3600).round();
```

---

## GÖREV 3 — Sürücü ekranı: `picked_up` durumunda iki rota göster

**Dosya:** `lib/modules/ride_sharing/driver_home_page.dart`

### 3a. İki ayrı rota listesi ekle

`_DriverHomePageState` class'ına mevcut `_routePoints` yerine iki liste ekle:

```dart
// ESKİ:
List<LatLng> _routePoints = [];

// YENİ:
List<LatLng> _routeToPickup = [];    // sürücü → pickup (yeşil)
List<LatLng> _routeToDropoff = [];   // pickup → dropoff (mavi)
String _ridePhase = 'idle'; // idle, driving_to_pickup, picked_up
```

### 3b. `_fetchRoute()` metodunu ikiye ayır

```dart
/// Sürücü konumundan pickup'a rota çeker (yeşil)
Future<void> _fetchRouteToPickup(double pickupLat, double pickupLon) async {
  if (_currentLocation == null) return;
  final points = await _fetchRoutePoints(
    _currentLocation!.latitude, _currentLocation!.longitude,
    pickupLat, pickupLon,
  );
  setState(() => _routeToPickup = points);
}

/// Pickup'tan dropoff'a rota çeker (mavi)
Future<void> _fetchRouteToDropoff(
  double pickupLat, double pickupLon,
  double dropoffLat, double dropoffLon,
) async {
  final points = await _fetchRoutePoints(
    pickupLat, pickupLon,
    dropoffLat, dropoffLon,
  );
  setState(() => _routeToDropoff = points);
}

/// Backend'den rota noktalarını çeker
Future<List<LatLng>> _fetchRoutePoints(
  double startLat, double startLon,
  double endLat, double endLon,
) async {
  try {
    final url = Uri.parse(
      '${AppConfig.apiEndpoint}/ride/route'
      '?start_lat=$startLat&start_lon=$startLon'
      '&end_lat=$endLat&end_lon=$endLon',
    );
    final token = await AuthService().getAccessToken();
    final response = await http.get(url, headers: {
      'Accept': 'application/json',
      if (token != null) 'Authorization': 'Bearer $token',
    });

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body) as Map<String, dynamic>;
      final points = data['data'] as List?;
      if (points != null && points.length >= 2) {
        return points
            .map((p) => LatLng(
                  (p as Map)['lat'] as double,
                  p['lon'] as double,
                ))
            .toList();
      }
    }
  } catch (e) {
    debugPrint('DriverHomePage: rota çekilemedi: $e');
  }
  // Fallback
  return [
    LatLng(startLat, startLon),
    LatLng(endLat, endLon),
  ];
}
```

### 3c. `_acceptOffer()` içini güncelle

```dart
void _acceptOffer() {
  if (_pendingOffer == null) return;
  final rideId = _pendingOffer!['ride_id'] as int;
  _send({'type': 'offer_response', 'ride_id': rideId, 'accepted': true});
  _offerTimer?.cancel();

  final pickupLat = (_pendingOffer!['pickup_lat'] as num?)?.toDouble();
  final pickupLon = (_pendingOffer!['pickup_lon'] as num?)?.toDouble();
  final dropoffLat = (_pendingOffer!['dropoff_lat'] as num?)?.toDouble();
  final dropoffLon = (_pendingOffer!['dropoff_lon'] as num?)?.toDouble();

  setState(() {
    _activeRideInfo = _pendingOffer;
    _hasActiveRide = true;
    _ridePhase = 'driving_to_pickup';
    _pendingOffer = null;
    _routeToPickup = [];
    _routeToDropoff = [];
  });

  // Sürücü → pickup rotası (yeşil)
  if (pickupLat != null && pickupLon != null) {
    _fetchRouteToPickup(pickupLat, pickupLon);

    // Pickup → dropoff rotası (mavi)
    if (dropoffLat != null && dropoffLon != null) {
      _fetchRouteToDropoff(pickupLat, pickupLon, dropoffLat, dropoffLon);
    }

    // Haritayı pickup noktasına kaydır
    _mapController.move(LatLng(pickupLat, pickupLon), 13);
  }
}
```

### 3d. `picked_up` durumunda yeşil rotayı temizle

`_handleMessage` içindeki `ride_status_changed` handler'ına `picked_up` durumunu ekle:

```dart
case 'ride_status_changed':
  final status = msg['status'] as String?;
  if (status == 'accepted') {
    _startActiveRide(msg);
  } else if (status == 'picked_up') {
    // Pickup tamamlandı: yeşil rota kalkar, sürücü dropoff'a gider
    setState(() {
      _ridePhase = 'picked_up';
      _routeToPickup = []; // yeşil rota temizle
    });
    // Haritayı dropoff'a kaydır
    final dropoffLat = (_activeRideInfo?['dropoff_lat'] as num?)?.toDouble();
    final dropoffLon = (_activeRideInfo?['dropoff_lon'] as num?)?.toDouble();
    if (dropoffLat != null && dropoffLon != null) {
      _mapController.move(LatLng(dropoffLat, dropoffLon), 13);
    }
  } else if (status == 'cancelled' || status == 'completed' || status == 'no_driver') {
    _endActiveRide(status);
  }
```

### 3e. `_endActiveRide()` içinde rotaları temizle

```dart
void _endActiveRide(String? status) {
  _offerTimer?.cancel();
  setState(() {
    _hasActiveRide = false;
    _activeRideInfo = null;
    _routeToPickup = [];
    _routeToDropoff = [];
    _ridePhase = 'idle';
    _pendingOffer = null;
  });
  // SnackBar'ı kaldır — Görev 4'te dialog ile değiştirilecek
}
```

### 3f. Haritada iki rota çiz

`FlutterMap` `children` listesinde `PolylineLayer`'ı güncelle:

```dart
// ESKİ tek PolylineLayer yerine:
if (_routeToPickup.isNotEmpty)
  PolylineLayer(
    polylines: [
      Polyline(
        points: _routeToPickup,
        color: Colors.green,
        strokeWidth: 4,
      ),
    ],
  ),
if (_routeToDropoff.isNotEmpty)
  PolylineLayer(
    polylines: [
      Polyline(
        points: _routeToDropoff,
        color: Colors.blue,
        strokeWidth: 4,
      ),
    ],
  ),
```

---

## GÖREV 4 — SnackBar'ları dialog ile değiştir

`pubspec.yaml`'a ekle:
```yaml
awesome_dialog: ^3.2.0
```

`driver_home_page.dart`'a import ekle:
```dart
import 'package:awesome_dialog/awesome_dialog.dart';
```

### 4a. Teklif süresi doldu

```dart
// ESKİ:
ScaffoldMessenger.of(context).showSnackBar(
  const SnackBar(content: Text('Teklif süresi doldu')),
);

// YENİ:
AwesomeDialog(
  context: context,
  dialogType: DialogType.warning,
  animType: AnimType.scale,
  title: 'Teklif Süresi Doldu',
  desc: 'Bu yolculuk teklifi zaman aşımına uğradı.',
  btnOkText: 'Tamam',
  btnOkOnPress: () {},
  autoDissmiss: true,
  dismissOnTouchOutside: true,
).show();
```

### 4b. Yolculuk onaylandı

```dart
// _startActiveRide() içindeki SnackBar yerine:
AwesomeDialog(
  context: context,
  dialogType: DialogType.success,
  animType: AnimType.bottomSlide,
  title: 'Yolculuk Onaylandı!',
  desc: 'Yolcuya doğru ilerleyin.',
  btnOkText: 'Tamam',
  btnOkOnPress: () {},
  autoDissmiss: true,
  dismissOnTouchOutside: true,
).show();
```

### 4c. Yolculuk tamamlandı / iptal edildi

```dart
// _endActiveRide() içindeki SnackBar yerine:
void _endActiveRide(String? status) {
  _offerTimer?.cancel();
  setState(() {
    _hasActiveRide = false;
    _activeRideInfo = null;
    _routeToPickup = [];
    _routeToDropoff = [];
    _ridePhase = 'idle';
    _pendingOffer = null;
  });

  if (!mounted) return;

  if (status == 'cancelled') {
    AwesomeDialog(
      context: context,
      dialogType: DialogType.warning,
      animType: AnimType.scale,
      title: 'Yolcu İptal Etti',
      desc: 'Yolcu yolculuğu iptal etti.',
      btnOkText: 'Tamam',
      btnOkOnPress: () {},
      dismissOnTouchOutside: true,
    ).show();
  } else if (status == 'completed') {
    AwesomeDialog(
      context: context,
      dialogType: DialogType.success,
      animType: AnimType.bottomSlide,
      title: 'Yolculuk Tamamlandı 🎉',
      desc: 'Harika iş! Yeni teklif bekleniyor.',
      btnOkText: 'Devam',
      btnOkOnPress: () {},
      dismissOnTouchOutside: true,
    ).show();
  }
}
```

### 4d. Yolculuk bitirilemedi hatası

```dart
// _completeRide() içindeki SnackBar'ları değiştir:
AwesomeDialog(
  context: context,
  dialogType: DialogType.error,
  animType: AnimType.scale,
  title: 'Hata',
  desc: 'Yolculuk bitirilemedi, tekrar deneyin.',
  btnOkText: 'Tamam',
  btnOkOnPress: () {},
).show();
```

---

## GÖREV 5 — Yolcu ekranı: SnackBar'ları da düzelt

**Dosya:** `lib/modules/ride_sharing/home_page.dart`

`home_page.dart`'a import ekle:
```dart
import 'package:awesome_dialog/awesome_dialog.dart';
```

Mevcut `ScaffoldMessenger.showSnackBar` çağrılarını bul ve şunlarla değiştir:

**Rota hesaplanamadı:**
```dart
AwesomeDialog(
  context: context,
  dialogType: DialogType.warning,
  animType: AnimType.scale,
  title: 'Rota Hesaplanamadı',
  desc: 'Lütfen tekrar deneyin.',
  btnOkText: 'Tamam',
  btnOkOnPress: () {},
).show();
```

---

## Kurallar

- `flutter pub get` çalıştır
- `flutter analyze` temiz geçmeli
- Mevcut `_routePoints` değişkenini tamamen kaldır, tüm kullanımlarını `_routeToPickup` / `_routeToDropoff` ile değiştir
- `_fetchRoute()` metodunu tamamen kaldır, yerine `_fetchRouteToPickup()` ve `_fetchRouteToDropoff()` kullan
- `route_service.dart`'ta `searchAddress()` metoduna dokunma
- Backend `get_route` endpoint'inde mevcut fallback mantığına dokunma
