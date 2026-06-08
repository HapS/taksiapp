# Flutter Ride Modülü — Backend Entegrasyon Prompt

## Mevcut durumun özeti

Flutter uygulaması şu an çalışıyor:
- Login / Register / Profile ekranları var
- JWT token `flutter_secure_storage`'da saklanıyor (`access_token`, `refresh_token`)
- `AuthService` token yönetimini yapıyor, `Authorization: Bearer <token>` header'ı kullanıyor
- Harita ekranında ORS ile rota hesaplanıyor, "Taksi Çağır" butonu var ama sadece snackbar gösteriyor
- `AppConfig.apiBaseUrl = 'https://one.web.tr'` (production sunucu)
- Riverpod kullanılıyor (`rideProvider`, `authProvider`)
- Geocoding için ORS autocomplete kullanılıyor (`RouteService.searchAddress`)
- Rota için ORS directions kullanılıyor (`RouteService.getRoute`)

## Backend API'nin mevcut durumu

Backend Rust/Axum ile yazılmış, şu endpointler hazır:

```
POST /api/ride/request     → taksi talebi oluştur
GET  /api/ride/:id         → ride durumunu sorgula
WS   /ws/passenger?user_id=<id>   → yolcu WebSocket bağlantısı
```

**Önemli:** WebSocket şu an `user_id` query param alıyor (TODO: JWT ile değiştirilecek). Bu prompt kapsamında query param kullanmaya devam et.

Backend mesaj formatları:

```json
// Server → Client (ServerMessage)
{"type":"ride_offer", ...}
{"type":"driver_location", "ride_id": 1, "lat": 41.01, "lon": 28.97}
{"type":"ride_status_changed", "ride_id": 1, "status": "accepted"}
{"type":"offer_expired", "ride_id": 1}
{"type":"pong"}
{"type":"error", "message": "..."}

// Client → Server (ClientMessage)
{"type":"location_update", "lat": 41.01, "lon": 28.97}
{"type":"offer_response", "ride_id": 1, "accepted": true}
{"type":"ping"}
```

POST /api/ride/request body:
```json
{
  "user_id": 1,
  "pickup_lat": 41.015,
  "pickup_lon": 28.979,
  "pickup_address": "Taksim Meydanı",
  "dropoff_lat": 41.008,
  "dropoff_lon": 28.978,
  "dropoff_address": "Galataport"
}
```

Response:
```json
{"ride_id": 3, "status": "searching"}
```

GET /api/ride/:id response:
```json
{
  "id": 3,
  "status": "accepted",
  "driver": {
    "full_name": "Test Sürücü",
    "vehicle_plate": "34 TEST 001",
    "vehicle_model": "Toyota Corolla",
    "phone": "05551234567"
  },
  "distance_km": 1.2,
  "duration_sec": 180,
  "fare_amount": 45.00
}
```

---

## Yapılacaklar

### Görev 1 — RideService oluştur

`lib/modules/ride_sharing/services/ride_service.dart` dosyasını oluştur.

```dart
class RideService {
  static const String _baseUrl = AppConfig.apiEndpoint;

  // POST /api/ride/request
  // AuthService'den access token al, header'a ekle
  // user_id'yi AuthService üzerinden JWT'den parse et
  static Future<RideRequestResponse> requestRide({
    required double pickupLat,
    required double pickupLon,
    required String pickupAddress,
    required double dropoffLat,
    required double dropoffLon,
    required String dropoffAddress,
  }) async { ... }

  // GET /api/ride/:id
  static Future<RideStatusResponse> getRideStatus(int rideId) async { ... }
}
```

`RideRequestResponse` modeli:
```dart
class RideRequestResponse {
  final bool success;
  final int? rideId;
  final String? status;
  final String? error;
}
```

`RideStatusResponse` modeli:
```dart
class RideStatusResponse {
  final bool success;
  final int? id;
  final String? status;      // searching, accepted, picked_up, completed, cancelled, no_driver
  final DriverInfo? driver;
  final double? distanceKm;
  final int? durationSec;
  final double? fareAmount;
  final String? error;
}

class DriverInfo {
  final String fullName;
  final String vehiclePlate;
  final String vehicleModel;
  final String phone;
}
```

`user_id`'yi JWT'den parse et (AuthService'deki `isTokenExpired` metoduna bakarak aynı yöntemle JWT payload'ını decode et, `sub` veya `user_id` claim'ini al).

---

### Görev 2 — WebSocket service oluştur

`lib/modules/ride_sharing/services/ride_ws_service.dart` dosyasını oluştur.

```dart
import 'package:web_socket_channel/web_socket_channel.dart';

class RideWsService {
  static const String _wsBaseUrl = 'wss://one.web.tr';
  
  WebSocketChannel? _channel;
  StreamController<ServerMessage>? _messageController;
  
  Stream<ServerMessage> get messages => _messageController!.stream;
  
  // Bağlantı aç: /ws/passenger?user_id=<id>
  Future<void> connect(int userId) async { ... }
  
  // Ping gönder (bağlantıyı canlı tut, her 20sn)
  void startPingTimer() { ... }
  
  // Bağlantıyı kapat
  void disconnect() { ... }
}
```

Gelen JSON mesajları parse et, `ServerMessage` sealed class'a dönüştür:

```dart
sealed class ServerMessage {}
class RideOfferMessage extends ServerMessage { ... }
class DriverLocationMessage extends ServerMessage {
  final int rideId;
  final double lat;
  final double lon;
}
class RideStatusChangedMessage extends ServerMessage {
  final int rideId;
  final String status;
}
class OfferExpiredMessage extends ServerMessage { final int rideId; }
class PongMessage extends ServerMessage {}
class ErrorMessage extends ServerMessage { final String message; }
```

`pubspec.yaml`'a ekle (yoksa):
```yaml
web_socket_channel: ^2.4.0
```

---

### Görev 3 — RideState ve RideNotifier genişlet

`lib/modules/ride_sharing/providers/ride_provider.dart` dosyasını güncelle.

Mevcut `RideState`'e şu alanları ekle:

```dart
// Ride durumu
final int? activeRideId;
final String rideStatus;   // idle, searching, accepted, picked_up, completed, no_driver
final DriverInfo? assignedDriver;
final LatLng? driverLocation;   // sürücünün anlık konumu (WS'den gelir)
final double? fareAmount;
```

`RideNotifier`'a şu metodları ekle:

```dart
// Taksi çağır butonuna basılınca çağrılır
Future<void> requestRide() async {
  // state'deki currentLocation ve destination'ı kullan
  // RideService.requestRide() çağır
  // Başarılıysa state.rideStatus = 'searching', activeRideId set et
  // WS bağlantısını başlat
}

// WS mesajlarını dinle
void _handleWsMessage(ServerMessage msg) {
  // RideStatusChangedMessage → rideStatus güncelle
  // DriverLocationMessage → driverLocation güncelle
  // OfferExpiredMessage → gerekirse UI'a yansıt
}

// Ride iptal
Future<void> cancelRide() async { ... }

// Ride tamamlandı / no_driver → state sıfırla
void resetRide() { ... }
```

`RideWsService` instance'ını `RideNotifier` içinde tut, `requestRide()` başarılı olunca `connect()` çağır, dispose'da `disconnect()` çağır.

---

### Görev 4 — RideHomePage güncelle

`lib/modules/ride_sharing/home_page.dart` dosyasındaki "Taksi Çağır" butonunun `onPressed`'ini güncelle:

```dart
onPressed: () async {
  final notifier = ref.read(rideProvider.notifier);
  await notifier.requestRide();
},
```

Harita üzerine sürücü konumu marker'ı ekle (rideState.driverLocation != null ise):

```dart
if (rideState.driverLocation != null)
  MarkerLayer(
    markers: [
      Marker(
        point: rideState.driverLocation!,
        width: 40,
        height: 40,
        child: const Icon(Icons.local_taxi, color: Colors.yellow, size: 40),
      ),
    ],
  ),
```

Ekranın alt kısmında ride durumuna göre bir bilgi kartı göster:

```
rideStatus == 'searching'  → "Sürücü aranıyor..." + CircularProgressIndicator
rideStatus == 'accepted'   → Sürücü adı, plaka, model + "Sürücü yolda" 
rideStatus == 'picked_up'  → "Yolculuk başladı"
rideStatus == 'no_driver'  → "Yakında sürücü bulunamadı" + Tekrar dene butonu
rideStatus == 'completed'  → "Yolculuk tamamlandı, ücret: X TL" + Kapat butonu
```

Bu kart `Positioned` ile haritanın alt kısmına yerleştirilmeli (bottom: 0), beyaz arka plan, rounded top corners, shadow.

---

### Görev 5 — Düzenli durum sorgulama (polling fallback)

`RideNotifier` içine polling mekanizması ekle. WS bağlantısı kesilirse veya 10 saniyede bir `GET /api/ride/:id` çağrılsın, durum değişmişse state'i güncelle. `activeRideId` null olunca timer'ı durdur.

```dart
Timer? _pollTimer;

void _startPolling(int rideId) {
  _pollTimer = Timer.periodic(const Duration(seconds: 10), (_) async {
    final response = await RideService.getRideStatus(rideId);
    if (response.success && response.status != null) {
      // state güncelle
      if (response.status == 'completed' || response.status == 'no_driver') {
        _stopPolling();
      }
    }
  });
}

void _stopPolling() {
  _pollTimer?.cancel();
  _pollTimer = null;
}
```

---

### Görev 6 — AppConfig güncelleme

`lib/core/config/app_config.dart` dosyasına WebSocket URL ekle:

```dart
/// WebSocket Base URL
static const String wsBaseUrl = 'wss://one.web.tr';
```

---

## Kurallar

- Mevcut `AuthService`, `RouteService`, `GeocodingService` dosyalarına dokunma
- Mevcut `RideState` alanlarını silme, sadece ekle
- `AppConfig`'deki ORS key'i olduğu gibi bırak (ORS rota hesaplama Flutter'da kalmaya devam ediyor)
- Tüm HTTP isteklerinde `Authorization: Bearer <token>` header'ı ekle
- Hata durumlarında `ScaffoldMessenger` ile snackbar göster
- `debugPrint` ile önemli olayları logla (WS bağlantı, mesaj, hata)
- `pubspec.yaml`'a sadece `web_socket_channel` ekle, diğer paketlere dokunma
- `flutter pub get` sonrası `flutter analyze` temiz geçmeli
