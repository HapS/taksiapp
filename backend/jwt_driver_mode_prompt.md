# JWT Auth + Sürücü Modu + Fake Yolcu Botu — Agent Prompt

## Mevcut kod yapısı

```
rust_backend_ride_module/
├── controllers/ride.rs       — HTTP endpointler (user_id hala body'den geliyor)
├── dispatch.rs               — sürücü eşleştirme loop'u
├── entities/                 — SeaORM entity'ler
├── ws/handler.rs             — WS handler (driver_id/user_id query param'dan geliyor)
├── ws/hub.rs                 — DashMap tabanlı bağlantı yönetimi
└── ws/messages.rs            — mesaj tipleri

flutter_lib/modules/
├── auth/
│   ├── services/auth_service.dart   — JWT storage, login, refresh
│   ├── providers/auth_provider.dart — AuthState, AuthNotifier
│   └── models/                      — User (id, username, email, user_type YOK)
└── ride_sharing/
    ├── home_page.dart               — yolcu harita ekranı
    ├── providers/ride_provider.dart — RideNotifier, RideState
    ├── services/ride_service.dart   — HTTP + JWT, parseUserIdFromToken hazır
    └── services/ride_ws_service.dart — WS bağlantı yöneticisi

fake_driver_bot.py  — mevcut, JWT yokken çalışıyor
```

### Kritik bilgiler
- `AppConfig.apiEndpoint = 'https://one.web.tr/api'`
- `AppConfig.wsBaseUrl = 'wss://one.web.tr'`
- JWT payload'da `user_id` veya `sub` claim'i var (`parseUserIdFromToken` bunu parse ediyor)
- `AuthService.getAccessToken()` → `FlutterSecureStorage`'dan token döner
- Backend'de JWT doğrulama middleware'i var, `/api/auth/*` hariç tüm route'larda çalışıyor
- `users` tablosunda `user_type` kolonu var (`VARCHAR DEFAULT 'B2C'`)
- `drivers` tablosunda `user_id` FK var (users tablosuna bağlı)
- Backend'de mevcut auth modülünden `Claims` struct'ı ve JWT doğrulama fonksiyonu erişilebilir

---

## BÖLÜM 1 — BACKEND: JWT ile WS Kimlik Doğrulama

### Görev 1 — WS handler'larında query param → JWT token

**Dosya:** `src/modules/ride/ws/handler.rs`

Mevcut `DriverParams` ve `PassengerParams` struct'larını kaldır. Yerine JWT token'ı header'dan veya query param'dan al.

WebSocket upgrade sırasında Authorization header'ı destekleyen tarayıcılar yok, bu yüzden token'ı query param olarak al ama doğrula:

```rust
#[derive(serde::Deserialize)]
pub struct WsAuthParams {
    pub token: String,
}
```

`driver_ws_handler` fonksiyonunu güncelle:

```rust
pub async fn driver_ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsAuthParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // JWT doğrula — mevcut auth modülündeki fonksiyonu kullan
    // Proje genelinde kullanılan Claims struct ve verify fonksiyonunu import et
    let claims = match crate::modules::auth::jwt::verify_token(&params.token, &state.config.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return (axum::http::StatusCode::UNAUTHORIZED, "Geçersiz token").into_response();
        }
    };

    let user_id = claims.user_id; // i64

    // Bu user_id için drivers tablosunda kayıt var mı kontrol et
    use crate::modules::ride::entities::drivers::{self, Entity as Driver};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let driver = match Driver::find()
        .filter(drivers::Column::UserId.eq(user_id))
        .filter(drivers::Column::IsActive.eq(true))
        .one(&state.db)
        .await
    {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (axum::http::StatusCode::FORBIDDEN, "Sürücü kaydı bulunamadı").into_response();
        }
        Err(_) => {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "DB hatası").into_response();
        }
    };

    let driver_id = driver.id;

    ws.on_upgrade(move |socket| {
        handle_driver_socket(socket, driver_id, Arc::new(state))
    }).into_response()
}
```

`passenger_ws_handler` fonksiyonunu güncelle:

```rust
pub async fn passenger_ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsAuthParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let claims = match crate::modules::auth::jwt::verify_token(&params.token, &state.config.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return (axum::http::StatusCode::UNAUTHORIZED, "Geçersiz token").into_response();
        }
    };

    let user_id = claims.user_id;

    ws.on_upgrade(move |socket| {
        handle_passenger_socket(socket, user_id, Arc::new(state))
    }).into_response()
}
```

> **NOT:** Mevcut projede JWT doğrulama fonksiyonunun tam import yolu farklı olabilir. `src/modules/auth/` altında `jwt.rs` veya benzeri bir dosyayı bul, `verify_token` veya `decode_token` fonksiyonunu ve `Claims` struct'ını kullan. `Claims.user_id` alanının adı farklıysa (örn. `sub`) ona göre düzenle.

---

### Görev 2 — `request_ride` controller'da user_id JWT'den alınsın

**Dosya:** `src/modules/ride/controllers/ride.rs`

`RideRequest` struct'ından `user_id` alanını kaldır:

```rust
// ESKİ:
pub struct RideRequest {
    pub user_id: i64,  // kaldır
    pub pickup_lat: f64,
    // ...
}

// YENİ:
pub struct RideRequest {
    pub pickup_lat: f64,
    pub pickup_lon: f64,
    pub pickup_address: String,
    pub dropoff_lat: f64,
    pub dropoff_lon: f64,
    pub dropoff_address: String,
}
```

`request_ride` handler'ına JWT extract ekle:

```rust
pub async fn request_ride(
    State(state): State<AppState>,
    // Mevcut projede kullanılan JWT extractor'ı kullan
    // Örneğin: Extension(claims): Extension<Claims>
    // veya: TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>
    // Projedeki diğer auth-korumalı handler'lara bak ve aynı pattern'i kullan
    claims: Claims,  // projedeki mevcut extractor
    Json(body): Json<RideRequest>,
) -> impl IntoResponse {
    let user_id = claims.user_id; // i64

    let new_ride = rides::ActiveModel {
        user_id: Set(user_id),  // body.user_id yerine JWT'den gelen
        // ... geri kalan aynı
    };
    // ...
}
```

> **NOT:** Projede başka korumalı endpoint'ler varsa (örn. `/api/user/profile`) onlardaki JWT extractor kullanımına bak ve aynı pattern'i kopyala.

---

### Görev 3 — `drivers` tablosuna `user_type` kontrolü için migration

Backend'de `users.user_type = 'driver'` olanlar sürücü sayılacak. Mevcut migration convention'ına göre yeni migration:

```sql
-- users tablosuna yeni user_type değeri eklenmesi enum değilse constraint'e gerek yok
-- Sadece test verisi için:
-- UPDATE users SET user_type = 'driver' WHERE id = <sürücü_user_id>;
```

Migration dosyası OLUŞTURMA — mevcut `migration/` klasöründeki convention'a bak (m20240101_000001 formatı gibi), aşağıdaki SQL'i ekle:

```sql
-- Sürücü kullanıcıları için driver kaydı otomatik oluştur (opsiyonel helper)
-- Şimdilik sadece user_type constraint'ini belge

-- Bunu da ekle: drivers tablosuna user_id unique constraint yoksa ekle
ALTER TABLE drivers ADD CONSTRAINT IF NOT EXISTS drivers_user_id_unique UNIQUE (user_id);
```

---

## BÖLÜM 2 — FLUTTER: JWT ile WS Bağlantısı

### Görev 4 — `RideWsService.connect()` token ile bağlansın

**Dosya:** `lib/modules/ride_sharing/services/ride_ws_service.dart`

`connect(int userId)` → `connect()` olarak değiştir, token AuthService'den alınsın:

```dart
/// WebSocket bağlantısını JWT token ile açar.
/// Token'ı FlutterSecureStorage'dan alır, WS URL'ine ekler.
/// Backend: /ws/passenger?token=<jwt>
Future<void> connect() async {
  disconnect();
  _messageController = StreamController<ServerMessage>.broadcast();

  // JWT token'ı al
  final authService = AuthService();
  final token = await authService.getAccessToken();
  if (token == null) {
    debugPrint('RideWsService: token bulunamadı, bağlantı iptal');
    _messageController?.add(ErrorMessage('Token bulunamadı'));
    return;
  }

  final uri = Uri.parse('${AppConfig.wsBaseUrl}/ws/passenger?token=${Uri.encodeComponent(token)}');
  debugPrint('RideWsService: connecting to $uri');

  _channel = WebSocketChannel.connect(uri);

  try {
    await _channel!.ready;
    debugPrint('RideWsService: connection ready');
  } catch (e) {
    debugPrint('RideWsService: connection failed: $e');
    _messageController?.add(ErrorMessage('Bağlantı kurulamadı: $e'));
    return;
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

---

### Görev 5 — `RideNotifier._connectWs()` güncelle

**Dosya:** `lib/modules/ride_sharing/providers/ride_provider.dart`

`_connectWs()` metodunu güncelle — artık `userId` parametre almıyor:

```dart
Future<void> _connectWs() async {
  await _wsService.connect();  // token içten alınıyor
  _wsSub = _wsService.messages.listen(_handleWsMessage);
  debugPrint('RideNotifier: WS bağlandı');
}
```

---

### Görev 6 — `User` modeline `userType` ekle

**Dosya:** `lib/modules/auth/models/user_model.dart`

`User` class'ına `userType` alanı ekle:

```dart
class User {
  final int id;
  final String username;
  final String email;
  final String? firstName;
  final String? lastName;
  final String? birthDate;
  final UserProfile? profile;
  final DateTime? createdAt;
  final String userType;  // 'B2C', 'B2B', 'driver' — EKLE

  User({
    required this.id,
    required this.username,
    required this.email,
    this.firstName,
    this.lastName,
    this.birthDate,
    this.profile,
    this.createdAt,
    this.userType = 'B2C',  // default — EKLE
  });

  factory User.fromJson(Map<String, dynamic> json) {
    return User(
      id: json['id'] as int,
      username: json['username'] as String,
      email: json['email'] as String,
      firstName: json['first_name'] as String?,
      lastName: json['last_name'] as String?,
      birthDate: json['birth_date'] as String?,
      profile: json['profile'] != null
          ? UserProfile.fromJson(json['profile'] as Map<String, dynamic>)
          : null,
      createdAt: json['created_at'] != null
          ? DateTime.tryParse(json['created_at'] as String)
          : null,
      userType: json['user_type'] as String? ?? 'B2C',  // EKLE
    );
  }

  // isDriver getter ekle
  bool get isDriver => userType == 'driver';
}
```

---

### Görev 7 — Sürücü modu routing

**Dosya:** `lib/app_router.dart`

Login sonrası `user.isDriver` kontrolü ekle — sürücü ana ekranına yönlendir:

```dart
// Mevcut authenticated route yönlendirmesini bul (AuthStatus.authenticated case)
// ve şu şekilde güncelle:

case AuthStatus.authenticated:
  final user = authState.user;
  if (user?.isDriver == true) {
    return const DriverHomePage();  // Görev 8'de oluşturulacak
  }
  return const RideSharingHomePage();  // mevcut yolcu ekranı
```

---

## BÖLÜM 3 — FLUTTER: Sürücü Modu Ekranı

### Görev 8 — Sürücü ana ekranı oluştur

**Yeni dosya:** `lib/modules/ride_sharing/driver_home_page.dart`

```dart
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:geolocator/geolocator.dart';
import 'dart:async';
import 'dart:convert';
import 'package:web_socket_channel/web_socket_channel.dart';
import '../../auth/services/auth_service.dart';
import '../../../core/config/app_config.dart';

class DriverHomePage extends ConsumerStatefulWidget {
  const DriverHomePage({super.key});

  @override
  ConsumerState<DriverHomePage> createState() => _DriverHomePageState();
}

class _DriverHomePageState extends ConsumerState<DriverHomePage> {
  final MapController _mapController = MapController();

  // Sürücü durumu
  bool _isOnline = false;
  LatLng? _currentLocation;

  // WS
  WebSocketChannel? _wsChannel;
  Timer? _pingTimer;
  Timer? _locationTimer;

  // Aktif teklif
  Map<String, dynamic>? _pendingOffer;
  Timer? _offerTimer;
  int _offerCountdown = 30;

  @override
  void initState() {
    super.initState();
    _initLocation();
  }

  @override
  void dispose() {
    _disconnect();
    _locationTimer?.cancel();
    _offerTimer?.cancel();
    super.dispose();
  }

  // Konum izni al ve ilk konumu belirle
  Future<void> _initLocation() async {
    LocationPermission perm = await Geolocator.checkPermission();
    if (perm == LocationPermission.denied) {
      perm = await Geolocator.requestPermission();
    }
    if (perm == LocationPermission.deniedForever) return;

    final pos = await Geolocator.getCurrentPosition();
    setState(() {
      _currentLocation = LatLng(pos.latitude, pos.longitude);
    });
    _mapController.move(_currentLocation!, 15);
  }

  // Online/Offline toggle
  Future<void> _toggleOnline() async {
    if (_isOnline) {
      _disconnect();
      setState(() { _isOnline = false; });
    } else {
      await _connect();
      setState(() { _isOnline = true; });
    }
  }

  // WS bağlantısı aç (token ile)
  Future<void> _connect() async {
    final token = await AuthService().getAccessToken();
    if (token == null) return;

    final uri = Uri.parse(
      '${AppConfig.wsBaseUrl}/ws/driver?token=${Uri.encodeComponent(token)}',
    );
    debugPrint('DriverWS: connecting to $uri');

    _wsChannel = WebSocketChannel.connect(uri);

    try {
      await _wsChannel!.ready;
      debugPrint('DriverWS: connected');
    } catch (e) {
      debugPrint('DriverWS: connection failed: $e');
      setState(() { _isOnline = false; });
      return;
    }

    _wsChannel!.stream.listen(
      (data) => _handleMessage(data as String),
      onError: (e) {
        debugPrint('DriverWS: error: $e');
        setState(() { _isOnline = false; });
      },
      onDone: () {
        debugPrint('DriverWS: closed');
        setState(() { _isOnline = false; });
      },
    );

    // Ping timer
    _pingTimer = Timer.periodic(const Duration(seconds: 20), (_) {
      _send({'type': 'ping'});
    });

    // Konum gönderme timer (her 3 saniye)
    _locationTimer = Timer.periodic(const Duration(seconds: 3), (_) async {
      try {
        final pos = await Geolocator.getCurrentPosition();
        final loc = LatLng(pos.latitude, pos.longitude);
        setState(() { _currentLocation = loc; });
        _send({'type': 'location_update', 'lat': loc.latitude, 'lon': loc.longitude});
      } catch (e) {
        debugPrint('DriverWS: konum alınamadı: $e');
      }
    });
  }

  // WS bağlantısını kapat
  void _disconnect() {
    _pingTimer?.cancel();
    _pingTimer = null;
    _locationTimer?.cancel();
    _locationTimer = null;
    _wsChannel?.sink.close();
    _wsChannel = null;
  }

  // Mesaj gönder
  void _send(Map<String, dynamic> data) {
    try {
      _wsChannel?.sink.add(jsonEncode(data));
    } catch (e) {
      debugPrint('DriverWS: send error: $e');
    }
  }

  // Gelen mesajları işle
  void _handleMessage(String raw) {
    debugPrint('DriverWS: received: $raw');
    try {
      final msg = jsonDecode(raw) as Map<String, dynamic>;
      final type = msg['type'] as String?;

      switch (type) {
        case 'ride_offer':
          _showOfferDialog(msg);
        case 'ride_status_changed':
          final status = msg['status'] as String?;
          debugPrint('DriverWS: ride status changed: $status');
          if (status == 'cancelled' || status == 'completed') {
            _offerTimer?.cancel();
            setState(() { _pendingOffer = null; });
            if (mounted) {
              ScaffoldMessenger.of(context).showSnackBar(
                SnackBar(
                  content: Text(
                    status == 'cancelled' ? 'Yolcu iptal etti' : 'Yolculuk tamamlandı 🎉',
                  ),
                ),
              );
            }
          }
        case 'pong':
          break;
        default:
          debugPrint('DriverWS: bilinmeyen mesaj tipi: $type');
      }
    } catch (e) {
      debugPrint('DriverWS: parse error: $e');
    }
  }

  // Teklif popup'ı göster
  void _showOfferDialog(Map<String, dynamic> offer) {
    setState(() {
      _pendingOffer = offer;
      _offerCountdown = 30;
    });

    // 30 saniye geri sayım
    _offerTimer?.cancel();
    _offerTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (_offerCountdown <= 1) {
        timer.cancel();
        setState(() { _pendingOffer = null; });
      } else {
        setState(() { _offerCountdown--; });
      }
    });
  }

  // Teklifi kabul et
  void _acceptOffer() {
    if (_pendingOffer == null) return;
    final rideId = _pendingOffer!['ride_id'] as int;
    _send({'type': 'offer_response', 'ride_id': rideId, 'accepted': true});
    _offerTimer?.cancel();
    setState(() { _pendingOffer = null; });
    debugPrint('DriverWS: teklif kabul edildi ride_id=$rideId');
  }

  // Teklifi reddet
  void _rejectOffer() {
    if (_pendingOffer == null) return;
    final rideId = _pendingOffer!['ride_id'] as int;
    _send({'type': 'offer_response', 'ride_id': rideId, 'accepted': false});
    _offerTimer?.cancel();
    setState(() { _pendingOffer = null; });
    debugPrint('DriverWS: teklif reddedildi ride_id=$rideId');
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Stack(
        children: [
          // Harita
          FlutterMap(
            mapController: _mapController,
            options: MapOptions(
              initialCenter: _currentLocation ?? const LatLng(40.772411, 30.363073),
              initialZoom: 15,
            ),
            children: [
              TileLayer(
                urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
                userAgentPackageName: 'com.example.app',
              ),
              if (_currentLocation != null)
                MarkerLayer(
                  markers: [
                    Marker(
                      point: _currentLocation!,
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
            ],
          ),

          // Üst bar
          Positioned(
            top: MediaQuery.of(context).padding.top + 8,
            left: 16,
            right: 16,
            child: Row(
              children: [
                // Online/Offline butonu
                Expanded(
                  child: GestureDetector(
                    onTap: _toggleOnline,
                    child: Container(
                      padding: const EdgeInsets.symmetric(vertical: 14),
                      decoration: BoxDecoration(
                        color: _isOnline ? Colors.green : Colors.grey[800],
                        borderRadius: BorderRadius.circular(12),
                        boxShadow: [
                          BoxShadow(
                            color: Colors.black.withAlpha(50),
                            blurRadius: 8,
                          ),
                        ],
                      ),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Icon(
                            _isOnline ? Icons.wifi : Icons.wifi_off,
                            color: Colors.white,
                          ),
                          const SizedBox(width: 8),
                          Text(
                            _isOnline ? 'Çevrimiçi — Teklif Bekleniyor' : 'Çevrimdışı',
                            style: const TextStyle(
                              color: Colors.white,
                              fontWeight: FontWeight.bold,
                              fontSize: 15,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),

          // Teklif kartı (gelen ride offer)
          if (_pendingOffer != null)
            Positioned(
              bottom: 0,
              left: 0,
              right: 0,
              child: Container(
                margin: const EdgeInsets.all(16),
                padding: const EdgeInsets.all(20),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(20),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withAlpha(50),
                      blurRadius: 20,
                      offset: const Offset(0, -4),
                    ),
                  ],
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        const Icon(Icons.local_taxi, color: Colors.amber, size: 28),
                        const SizedBox(width: 8),
                        const Text(
                          'Yeni Yolculuk Teklifi',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const Spacer(),
                        // Geri sayım
                        Container(
                          width: 40,
                          height: 40,
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            border: Border.all(color: Colors.orange, width: 2),
                          ),
                          child: Center(
                            child: Text(
                              '$_offerCountdown',
                              style: const TextStyle(
                                fontWeight: FontWeight.bold,
                                color: Colors.orange,
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),
                    // Pickup
                    Row(
                      children: [
                        const Icon(Icons.circle, color: Colors.green, size: 14),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            _pendingOffer!['pickup_address'] as String? ?? '?',
                            style: const TextStyle(fontSize: 14),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    // Dropoff
                    Row(
                      children: [
                        const Icon(Icons.location_on, color: Colors.red, size: 14),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            _pendingOffer!['dropoff_address'] as String? ?? '?',
                            style: const TextStyle(fontSize: 14),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    // Mesafe ve ücret
                    Row(
                      children: [
                        const Icon(Icons.route, color: Colors.grey, size: 14),
                        const SizedBox(width: 4),
                        Text(
                          '${((_pendingOffer!['distance_km'] as num?)?.toStringAsFixed(1)) ?? '?'} km',
                          style: const TextStyle(color: Colors.grey, fontSize: 13),
                        ),
                        const SizedBox(width: 16),
                        const Icon(Icons.access_time, color: Colors.grey, size: 14),
                        const SizedBox(width: 4),
                        Text(
                          '${(((_pendingOffer!['expires_in_secs'] as num?) ?? 30).toInt())} sn',
                          style: const TextStyle(color: Colors.grey, fontSize: 13),
                        ),
                      ],
                    ),
                    const SizedBox(height: 20),
                    // Kabul / Red butonları
                    Row(
                      children: [
                        Expanded(
                          child: OutlinedButton(
                            onPressed: _rejectOffer,
                            style: OutlinedButton.styleFrom(
                              foregroundColor: Colors.red,
                              side: const BorderSide(color: Colors.red),
                              padding: const EdgeInsets.symmetric(vertical: 14),
                              shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(10),
                              ),
                            ),
                            child: const Text(
                              'Reddet',
                              style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                            ),
                          ),
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          flex: 2,
                          child: ElevatedButton(
                            onPressed: _acceptOffer,
                            style: ElevatedButton.styleFrom(
                              backgroundColor: Colors.green,
                              foregroundColor: Colors.white,
                              padding: const EdgeInsets.symmetric(vertical: 14),
                              shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(10),
                              ),
                            ),
                            child: const Text(
                              'Kabul Et',
                              style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}
```

---

### Görev 9 — `app_router.dart` güncellemesi

**Dosya:** `lib/app_router.dart`

`DriverHomePage` import ekle ve routing güncelle:

```dart
import 'modules/ride_sharing/driver_home_page.dart';

// Mevcut routing logic'te authenticated case'i güncelle:
// authState.user?.isDriver kontrolü ekle
```

---

## BÖLÜM 4 — FAKE YOLCU BOTU

### Görev 10 — `fake_passenger_bot.py` oluştur

**Yeni dosya:** `fake_passenger_bot.py`

```python
#!/usr/bin/env python3
"""
Fake Yolcu Botu
- Backend'e login olur, JWT alır
- POST /api/ride/request ile taksi çağırır
- WS /ws/passenger?token=<jwt> ile bağlanır
- Sürücü kabul edince bekler, completed olunca tekrar çağırır
- 3-4 paralel yolcu simüle eder
"""

import asyncio
import json
import math
import random
import signal
import sys
import urllib.request
import urllib.error
import urllib.parse
import websockets

# --- Ayarlar ---
API_BASE = "https://one.web.tr/api"
WS_BASE  = "wss://one.web.tr"

# Test yolcuları: (username, password, pickup bölgesi merkezi)
# Bu kullanıcıların DB'de kayıtlı olması gerekiyor
PASSENGERS = [
    ("yolcu1", "password123", (40.7604, 30.3629)),
    ("yolcu2", "password123", (40.7750, 30.3800)),
    ("yolcu3", "password123", (40.7450, 30.3500)),
    ("yolcu4", "password123", (40.7680, 30.3720)),
]

# Sakarya bölgesindeki popüler noktalar (pickup/dropoff için)
LOCATIONS = [
    (40.7604062, 30.3629614, "Adapazarı Merkez"),
    (40.7750000, 30.3800000, "Serdivan"),
    (40.7450000, 30.3500000, "Arifiye"),
    (40.7680000, 30.3720000, "Mithatpaşa"),
    (40.7520000, 30.3650000, "Yeşiltepe"),
    (40.7830000, 30.3900000, "Erenler"),
]

RIDE_INTERVAL = (30, 90)  # başarılı yolculuktan sonra kaç saniye bekle
WAIT_TIMEOUT  = 120       # sürücü bulunamazsa kaç saniye sonra vazgeç
# ----------------


def log(prefix: str, msg: str):
    print(f"[{prefix}] {msg}", flush=True)


def http_post(url: str, body: dict, headers: dict = None) -> tuple[bool, int, str]:
    data = json.dumps(body).encode()
    h = {"Content-Type": "application/json"}
    if headers:
        h.update(headers)
    req = urllib.request.Request(url, data=data, headers=h, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


def http_get(url: str, token: str = None) -> tuple[bool, int, str]:
    h = {"Accept": "application/json"}
    if token:
        h["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


async def login(username: str, password: str) -> str | None:
    """Login ol, access token döndür."""
    url = f"{API_BASE}/auth/login"
    ok, code, body = await asyncio.to_thread(
        http_post, url, {"username": username, "password": password}
    )
    if not ok:
        return None
    try:
        data = json.loads(body)
        if data.get("success"):
            return data["tokens"]["access_token"]
    except Exception:
        pass
    return None


async def request_ride(token: str, pickup: tuple, dropoff: tuple, dropoff_name: str, pickup_name: str) -> int | None:
    """Taksi çağır, ride_id döndür."""
    url = f"{API_BASE}/ride/request"
    body = {
        "pickup_lat":      pickup[0],
        "pickup_lon":      pickup[1],
        "pickup_address":  pickup_name,
        "dropoff_lat":     dropoff[0],
        "dropoff_lon":     dropoff[1],
        "dropoff_address": dropoff_name,
    }
    headers = {"Authorization": f"Bearer {token}"}
    ok, code, resp_body = await asyncio.to_thread(http_post, url, body, headers)
    if not ok:
        return None
    try:
        data = json.loads(resp_body)
        return data.get("ride_id")
    except Exception:
        return None


async def run_passenger(username: str, password: str, home: tuple, tag: str):
    """Tek yolcu simülasyonu — sonsuz döngü."""
    log(tag, f"Başlatılıyor: {username}")

    # Login
    token = await login(username, password)
    if not token:
        log(tag, f"❌ Login başarısız: {username}")
        return

    log(tag, f"✅ Login: {username}")

    while True:
        # Rastgele pickup ve dropoff seç
        loc_list = LOCATIONS.copy()
        pickup_loc = random.choice(loc_list)
        loc_list.remove(pickup_loc)
        dropoff_loc = random.choice(loc_list)

        log(tag, f"Taksi çağırılıyor: {pickup_loc[2]} → {dropoff_loc[2]}")

        ride_id = await request_ride(
            token,
            (pickup_loc[0], pickup_loc[1]),
            (dropoff_loc[0], dropoff_loc[1]),
            dropoff_loc[2],
            pickup_loc[2],
        )

        if not ride_id:
            log(tag, "❌ Ride isteği başarısız. 30sn sonra tekrar denenecek...")
            await asyncio.sleep(30)
            continue

        log(tag, f"✅ Ride #{ride_id} oluşturuldu, WS bağlanılıyor...")

        # WS bağlan ve bekle
        ws_url = f"{WS_BASE}/ws/passenger?token={urllib.parse.quote(token)}"
        try:
            async with websockets.connect(ws_url) as ws:
                log(tag, f"WS bağlantısı kuruldu (ride #{ride_id})")

                # Ping timer
                async def ping_loop():
                    while True:
                        await asyncio.sleep(20)
                        try:
                            await ws.send(json.dumps({"type": "ping"}))
                        except Exception:
                            break

                ping_task = asyncio.create_task(ping_loop())
                ride_done = False
                start_time = asyncio.get_event_loop().time()

                async for raw in ws:
                    elapsed = asyncio.get_event_loop().time() - start_time
                    if elapsed > WAIT_TIMEOUT:
                        log(tag, f"⏱ Timeout ({WAIT_TIMEOUT}sn), yeni ride denenecek...")
                        break

                    try:
                        msg = json.loads(raw)
                    except Exception:
                        continue

                    mtype = msg.get("type")
                    if mtype == "ride_status_changed":
                        status = msg.get("status")
                        log(tag, f"Ride #{ride_id} durum: {status}")
                        if status == "accepted":
                            log(tag, "🚕 Sürücü kabul etti, bekleniyor...")
                        elif status == "picked_up":
                            log(tag, "🚗 Yolculuk başladı!")
                        elif status == "completed":
                            log(tag, "🎉 Yolculuk tamamlandı!")
                            ride_done = True
                            break
                        elif status in ("cancelled", "no_driver"):
                            log(tag, f"⚠️ Ride bitti: {status}")
                            break
                    elif mtype == "driver_location":
                        lat = msg.get("lat", 0)
                        lon = msg.get("lon", 0)
                        log(tag, f"📍 Sürücü konumu: ({lat:.4f}, {lon:.4f})")

                ping_task.cancel()

                wait_secs = random.randint(*RIDE_INTERVAL)
                if ride_done:
                    log(tag, f"✅ Tamamlandı. {wait_secs}sn sonra yeni ride...")
                else:
                    log(tag, f"Ride bitti/zaman aşımı. {wait_secs}sn sonra yeni ride...")
                await asyncio.sleep(wait_secs)

        except websockets.exceptions.ConnectionClosedError as e:
            log(tag, f"WS bağlantısı kesildi: {e}. 10sn sonra tekrar...")
            await asyncio.sleep(10)
        except Exception as e:
            log(tag, f"Beklenmedik hata: {e}. 15sn sonra tekrar...")
            await asyncio.sleep(15)


async def main():
    tasks = []
    for i, (username, password, home) in enumerate(PASSENGERS):
        tag = f"Y{i+1}"
        # Her yolcuyu farklı zamanda başlat
        await asyncio.sleep(i * 5)
        tasks.append(asyncio.create_task(
            run_passenger(username, password, home, tag)
        ))
    await asyncio.gather(*tasks)


if __name__ == "__main__":
    def handle_exit(sig, frame):
        print("[BOT] Durduruluyor...")
        sys.exit(0)

    signal.signal(signal.SIGINT, handle_exit)
    signal.signal(signal.SIGTERM, handle_exit)

    asyncio.run(main())
```

---

### Görev 11 — Test kullanıcıları DB'ye ekle

Aşağıdaki SQL'i çalıştır (password hash'i mevcut backend'in kullandığı hash algoritmasına göre üret — bcrypt ise bcrypt, argon2 ise argon2):

```sql
-- Önce user_type = 'driver' olan test sürücüsü (zaten varsa atla)
-- Sonra yolcu kullanıcıları
INSERT INTO users (username, email, password, user_type)
VALUES
  ('yolcu1', 'yolcu1@test.com', '<hashed_password>', 'B2C'),
  ('yolcu2', 'yolcu2@test.com', '<hashed_password>', 'B2C'),
  ('yolcu3', 'yolcu3@test.com', '<hashed_password>', 'B2C'),
  ('yolcu4', 'yolcu4@test.com', '<hashed_password>', 'B2C')
ON CONFLICT (username) DO NOTHING;
```

> Password hash için mevcut backend'de `bcrypt` veya `argon2` kullanıldığını kontrol et. Test için basit bir Rust script'i veya mevcut register endpoint'i kullanılabilir:
> ```bash
> curl -X POST https://one.web.tr/api/auth/register \
>   -H "Content-Type: application/json" \
>   -d '{"username":"yolcu1","password":"password123","email":"yolcu1@test.com"}'
> ```

---

## Kurallar

- Backend için `cargo check` geçmeli
- Flutter için `flutter analyze` temiz geçmeli
- `fake_passenger_bot.py` için: `python3 -m py_compile fake_passenger_bot.py`
- JWT verify fonksiyonu için mevcut backend auth modülünü kullan — yeni bir implementasyon yazma
- `Claims` struct'ındaki `user_id` field adı farklıysa (örn. `sub`, `id`) bul ve düzenle
- Flutter'da `geolocator` paketi zaten `pubspec.yaml`'da varsa eklemege gerek yok, yoksa ekle
- `DriverHomePage` ayrı bir dosyada olmalı, mevcut `home_page.dart`'a dokunma
- Fake yolcu botu sadece test içindir, production'da kullanılmaz
